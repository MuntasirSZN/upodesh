use std::{
    collections::{BTreeMap, HashSet},
    env::{args, var_os},
    fs::{self, read_to_string, File},
    io::BufWriter,
    path::PathBuf,
};

use fst::raw::Builder;
use fst::MapBuilder;
use simd_normalizer::UnicodeNormalization;

fn generate_words_fst() {
    let root = PathBuf::from(var_os("CARGO_MANIFEST_DIR").unwrap());
    let parent = root.parent().unwrap();
    let dest = parent.join("src").join("words.fst");
    let file = File::create(dest).expect("Failed to create words.fst");
    let writer = BufWriter::new(file);
    let mut fst = Builder::new(writer).unwrap();
    let words_dir = parent.join("data/openbangla-keyboard/data");
    let words = read_to_string(words_dir.join("dictionary.json")).expect("Failed to read");
    let mut words = words.lines().filter(|s| !s.is_empty()).collect::<Vec<_>>();
    words.sort();
    for word in words {
        let numbered = word.chars().map(|c| c as u8).collect::<Vec<u8>>();
        fst.add(&numbered).expect("add word");
    }
    fst.finish().expect("finish");
}

fn generate_typo_index() {
    const MAX_EDIT_DISTANCE: usize = 1;
    const PREFIX_LENGTH: usize = 7;
    let root = PathBuf::from(var_os("CARGO_MANIFEST_DIR").unwrap());
    let parent = root.parent().unwrap();
    let dest = parent.join("src").join("typo");
    fs::create_dir_all(&dest).expect("create typo dir");
    let words_dir = parent.join("data/openbangla-keyboard/data");
    let text = read_to_string(words_dir.join("dictionary.json")).expect("read");
    let mut words: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let w: String = line.trim().nfc().into_owned();
        if !w.is_empty() && seen.insert(w.clone()) {
            words.push(w);
        }
    }

    fn add_deletes(chars: &[char], level: usize, max: usize, out: &mut HashSet<Vec<char>>) {
        if level == max {
            return;
        }
        if chars.len() > 1 {
            for i in 0..chars.len() {
                let mut d = Vec::with_capacity(chars.len() - 1);
                d.extend_from_slice(&chars[..i]);
                d.extend_from_slice(&chars[i + 1..]);
                if out.insert(d.clone()) {
                    add_deletes(&d, level + 1, max, out);
                }
            }
        }
    }

    let mut deletes: BTreeMap<Vec<u8>, Vec<u32>> = BTreeMap::new();
    for (id, word) in words.iter().enumerate() {
        let prefix: Vec<char> = word.chars().take(PREFIX_LENGTH).collect();
        let mut dset: HashSet<Vec<char>> = HashSet::new();
        dset.insert(prefix.clone());
        add_deletes(&prefix, 0, MAX_EDIT_DISTANCE, &mut dset);
        for d in dset {
            let key: Vec<u8> = d.iter().map(|c| *c as u8).collect();
            deletes.entry(key).or_default().push(id as u32);
        }
    }

    let mut builder = MapBuilder::memory();
    let mut packed: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    for (key, ids) in &deletes {
        for id in ids {
            packed.extend_from_slice(&id.to_le_bytes());
        }
        let value = ((offset as u64) << 32) | (ids.len() as u64);
        offset += ids.len() as u32;
        builder.insert(key, value).expect("insert key");
    }
    let fst_bytes = builder.into_inner().expect("finish map");
    fs::write(dest.join("deletes.fst"), &fst_bytes).expect("write deletes.fst");
    fs::write(dest.join("word_ids.bin"), &packed).expect("write word_ids.bin");

    let mut blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<u8> = Vec::new();
    for w in &words {
        offsets.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        blob.extend_from_slice(w.as_bytes());
    }
    offsets.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    fs::write(dest.join("words.bin"), &blob).expect("write words.bin");
    fs::write(dest.join("word_offsets.bin"), &offsets).expect("write word_offsets.bin");
}

fn main() {
    generate_words_fst();
    generate_typo_index();
}
