pub mod affix;
use once_cell::sync::Lazy;
use symspell_rs::{SymSpell, Verbosity};

static SYM: Lazy<SymSpell> = Lazy::new(|| {
    let mut s = SymSpell::new(1, None, 7, 1);
    for line in include_str!("../data/openbangla-keyboard/data/dictionary.json").lines() {
        let w = line.trim();
        if !w.is_empty() {
            s.load_dictionary_line(&format!("{} 1", w), 0, 1, " ");
        }
    }
    s
});

pub fn suggest(input: &str) -> Vec<String> {
    SYM.lookup(input.trim(), Verbosity::Closest, 1, &None, Some(10), false)
        .into_iter()
        .map(|r| r.term)
        .collect()
}

pub fn suggest_with(input: &str, max_results: usize) -> Vec<String> {
    SYM.lookup(
        input.trim(),
        Verbosity::Closest,
        1,
        &None,
        Some(max_results),
        false,
    )
    .into_iter()
    .map(|r| r.term)
    .collect()
}
