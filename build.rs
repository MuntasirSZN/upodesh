//! Build-time construction of the embedded correction index.
//!
//! The runtime crate must not parse the 4 MB upstream dictionary or build an
//! FST on first use. Instead this script performs the same normalization,
//! grapheme segmentation and FST construction that `Corrector::from_entries`
//! does, then writes a self-contained blob that `Corrector::from_bytes` maps
//! back with a single `fst::Map::new` call.
//!
//! Keep the encoding in sync with `src/typo.rs` (`from_entries`/`from_bytes`).
//! `tests::embedded_index_matches_in_memory_build` guards the two paths.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
};

use fst::{Map, MapBuilder, Streamer};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const MAGIC: &[u8; 8] = b"UPDSHIX1";

fn normalize(word: &str) -> String {
    word.nfc().collect()
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dictionary = manifest.join("data/openbangla-keyboard/data/dictionary.json");
    let frequencies = manifest.join("data/frequency.json");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("dictionary.index");

    println!("cargo:rerun-if-changed={}", dictionary.display());
    println!("cargo:rerun-if-changed={}", frequencies.display());
    println!("cargo:rerun-if-changed=build.rs");

    let text = fs::read_to_string(&dictionary).expect("read upstream dictionary.json");
    let root: serde_json::Value = serde_json::from_str(&text).expect("dictionary.json is JSON");

    // Optional corpus frequencies. The upstream repo ships only a word list,
    // so this file is the injection point for a real frequency table.
    let frequency_table: BTreeMap<String, u64> = fs::read_to_string(&frequencies)
        .ok()
        .map(|text| {
            serde_json::from_str(&text).expect("data/frequency.json is a word-to-count object")
        })
        .unwrap_or_default();

    let mut entries = BTreeMap::<String, u64>::new();
    if let Some(buckets) = root.as_object() {
        for words in buckets.values() {
            let Some(words) = words.as_array() else {
                continue;
            };
            for word in words.iter().filter_map(serde_json::Value::as_str) {
                let word = normalize(word.trim());
                if word.is_empty() {
                    continue;
                }
                let frequency = frequency_table.get(&word).copied().unwrap_or(1).max(1);
                let slot = entries.entry(word).or_default();
                *slot = slot.saturating_add(frequency);
            }
        }
    }
    assert!(!entries.is_empty(), "dictionary contains no words");

    let graphemes: BTreeSet<String> = entries
        .keys()
        .flat_map(|word| UnicodeSegmentation::graphemes(word.as_str(), true).map(str::to_owned))
        .collect();
    assert!(
        graphemes.len() <= 65_536,
        "dictionary has more graphemes than the u16 key space"
    );
    let graphemes: Vec<String> = graphemes.into_iter().collect();
    let ids: BTreeMap<&str, u16> = graphemes
        .iter()
        .enumerate()
        .map(|(id, grapheme)| (grapheme.as_str(), id as u16))
        .collect();

    let mut encoded = Vec::<(Vec<u8>, u64)>::with_capacity(entries.len());
    for (word, frequency) in &entries {
        let mut key = Vec::with_capacity(word.len());
        for grapheme in UnicodeSegmentation::graphemes(word.as_str(), true) {
            let id = ids
                .get(grapheme)
                .copied()
                .expect("grapheme table covers every dictionary word");
            key.extend_from_slice(&id.to_be_bytes());
        }
        encoded.push((key, *frequency));
    }
    encoded.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let mut builder = MapBuilder::memory();
    for (key, frequency) in &encoded {
        builder
            .insert(key, *frequency)
            .expect("insert dictionary word");
    }
    let map = builder.into_map();
    let map_bytes = map.as_fst().as_bytes();

    let mut blob = Vec::with_capacity(8 + 4 + graphemes.len() * 8 + map_bytes.len());
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&(graphemes.len() as u32).to_le_bytes());
    for grapheme in &graphemes {
        blob.extend_from_slice(&(grapheme.len() as u32).to_le_bytes());
        blob.extend_from_slice(grapheme.as_bytes());
    }
    blob.extend_from_slice(&(map_bytes.len() as u32).to_le_bytes());
    blob.extend_from_slice(map_bytes);

    fs::write(&out, &blob).expect("write build-time dictionary index");

    // A cheap consistency probe: every encoded key must be present.
    let map = Map::new(map.as_fst().as_bytes().to_vec()).expect("reopen generated FST");
    let mut stream = map.stream();
    let mut count = 0_usize;
    while stream.next().is_some() {
        count += 1;
    }
    assert_eq!(count, encoded.len(), "FST entry count mismatch");
}
