//! Build-time construction of the embedded correction index (v2).
//!
//! The runtime `Corrector::from_bytes` maps one blob containing:
//! Bangla grapheme table + Bangla FST + two Roman tries (Avro, Khipro).
//! Roman tries are built from the pattern tables derived from okkhor/riti
//! rules (see `data/ATTRIBUTION.md`); see `src/typo/roman_gen.rs`.

#![allow(dead_code, clippy::all)]

include!("src/typo/roman_gen.rs");

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
};

use fst::{Map, MapBuilder, Streamer};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const MAGIC_V2: &[u8; 8] = b"UPDSHIX2";

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
    println!("cargo:rerun-if-changed={}", manifest.join("data/avro-patterns.json").display());
    println!("cargo:rerun-if-changed={}", manifest.join("data/khipro-patterns.json").display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/typo/roman_gen.rs");

    let text = fs::read_to_string(&dictionary).expect("read upstream dictionary.json");
    let root: serde_json::Value = serde_json::from_str(&text).expect("dictionary.json is JSON");

    let frequency_table: BTreeMap<String, u64> = fs::read_to_string(&frequencies)
        .ok()
        .map(|text| serde_json::from_str(&text).expect("frequency table is JSON"))
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
    let map_bytes = map.as_fst().as_bytes().to_vec();

    // ---- Roman tries for both modes -------------------------------
    let avro_bytes = fs::read(manifest.join("data/avro-patterns.json")).unwrap_or_default();
    let khipro_bytes =
        fs::read(manifest.join("data/khipro-patterns.json")).unwrap_or_default();
    let avro_set = load_patterns(&avro_bytes);
    let khipro_set = load_patterns(&khipro_bytes);

    let mut blob = Vec::new();
    blob.extend_from_slice(MAGIC_V2);
    blob.extend_from_slice(&(graphemes.len() as u32).to_le_bytes());
    for grapheme in &graphemes {
        blob.extend_from_slice(&(grapheme.len() as u32).to_le_bytes());
        blob.extend_from_slice(grapheme.as_bytes());
    }
    blob.extend_from_slice(&(map_bytes.len() as u32).to_le_bytes());
    blob.extend_from_slice(&map_bytes);

    // words blob + word offsets for roman terminal values
    let words: Vec<String> = entries.keys().cloned().collect();
    let mut words_blob = Vec::new();
    let mut word_off: BTreeMap<String, u32> = BTreeMap::new();
    for (i, w) in words.iter().enumerate() {
        let _ = i;
        word_off.insert(w.clone(), words_blob.len() as u32);
        words_blob.extend_from_slice(w.as_bytes());
    }
    for set in [&avro_set, &khipro_set] {
        // collect (form, off, len)
        let mut forms: Vec<(Vec<u8>, u32, u32)> = Vec::new();
        for w in &words {
            for f in set.forms(w) {
                if f.is_empty() {
                    continue;
                }
                let off = word_off[w.as_str()];
                forms.push((f.into_bytes(), off, w.len() as u32));
            }
        }
        forms.sort();
        forms.dedup_by(|a, b| a == b);
        // build byte trie CSR
        let mut nodes: Vec<(Vec<(u8, u32)>, Vec<(u32, u32)>)> = vec![(Vec::new(), Vec::new())];
        for (form, off, len) in &forms {
            let mut node = 0usize;
            for &b in form.iter() {
                let pos = nodes[node].0.iter().position(|(l, _)| *l == b);
                match pos {
                    Some(p) => node = nodes[node].0[p].1 as usize,
                    None => {
                        nodes.push((Vec::new(), Vec::new()));
                        let child = (nodes.len() - 1) as u32;
                        nodes[node].0.push((b, child));
                        node = child as usize;
                    }
                }
            }
            nodes[node].1.push((*off, *len));
        }
        let n = nodes.len();
        let (mut start, mut cnt, mut eid, mut ech, mut t_start, mut t_alloc, mut t_off, mut t_len);
        (start, cnt, eid, ech, t_start, t_alloc, t_off, t_len) = (vec![0u32; n], vec![0u32; n], Vec::new(), Vec::new(), vec![0u32; n], vec![0u32; n], Vec::new(), Vec::new());
        let total: usize = nodes.iter().map(|(c, _)| c.len()).sum();
        eid.reserve(total);
        ech.reserve(total);
        let mut offset = 0usize;
        let mut toffset = 0usize;
        for (i, (ch, terms)) in nodes.iter().enumerate() {
            start[i] = offset as u32;
            cnt[i] = ch.len() as u32;
            for &(b, c) in ch {
                eid.push(b);
                ech.push(c);
            }
            offset += ch.len();
            t_start[i] = toffset as u32;
            t_alloc[i] = terms.len() as u32;
            for &(o, l) in terms {
                t_off.push(o);
                t_len.push(l);
            }
            toffset += terms.len();
        }
        blob.extend_from_slice(&(n as u32).to_le_bytes());
        for v in [&start, &cnt] {
            for x in v {
                blob.extend_from_slice(&x.to_le_bytes());
            }
        }
        blob.extend_from_slice(&eid);
        for v in [&ech, &t_start, &t_alloc, &t_off, &t_len] {
            for x in v {
                blob.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
    blob.extend_from_slice(&(words_blob.len() as u32).to_le_bytes());
    blob.extend_from_slice(&words_blob);

    fs::write(&out, &blob).expect("write build-time dictionary index");

    let map = Map::new(map_bytes.clone()).expect("reopen generated FST");
    let mut stream = map.stream();
    let mut count = 0_usize;
    while stream.next().is_some() {
        count += 1;
    }
    assert_eq!(count, encoded.len(), "FST entry count mismatch");
}
