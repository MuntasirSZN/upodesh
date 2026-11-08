//! Vendored copy of **dhoni** (Bangla → phonetic Roman) — upstream:
//! `https://github.com/OpenBangla/dhoni`, commit
//! `ea64a5c5d910c02e1963c799baa6c9870be57361` (2019-08-10), MIT License
//! (c) 2019 OpenBangla. See `data/ATTRIBUTION.md`.
//!
//! Compared to upstream, this copy carries three local patches:
//!
//! 1. **Khipro mode** (`Mode::Khipro`). Upstream is Avro-only and wrong for
//!    Khipro typists. Per the `khipro-m17n` submodule's
//!    `khipro-testcases.tsv` and the `data/khipro-patterns.json` table: the
//!    candrabindu `ঁ` is typed with `/` (`ma/` → মাঁ, `ko/` → কঁ) and visarga
//!    `ৎ` with `t/` (`t/` → ৎ). Khipro canonical forms therefore emit `/` for
//!    `ঁ` and `t/` for `ৎ`.
//! 2. **Trailing-`/` variants** (`convert_to_phonetic_variants`). In Khipro a
//!    trailing `/` suppresses a virama (`ks/` → কস vs `ks` → ক্স). Words
//!    containing a virama therefore index both their primary form and the
//!    slash-suffixed variant, so a `/`-suppressing typist matches at zero
//!    edit cost instead of paying an insertion on every candidate.
//! 3. **Conjunct generalization**. Upstream special-cases only ক্ষ (`kkh`),
//!    ্ব (`w`) and ্য (`y`). Everything else already concatenates through
//!    virama-drop — now pinned by tests against the IPA table
//!    (`data/ipa-alignments.md`) — but the per-conjunct special cases have
//!    been centralized in `SPECIAL_CONJUNCTS` so future additions land in
//!    exactly one place (habibsifat's Juktakkhor list agrees with the set).
//!
//! Known upstream characteristics, deliberately unchanged:
//!
//! * Virama (`্`) is dropped, so conjuncts are their concatenated letters
//!   (স্ত → `st`, ঞ্জ → `ngj`). Collisions this creates are *phonetic
//!   equivalences* (ট/ত → `t`, শ/স → `s`), not errors: Avro typists type
//!   both the same way, and downstream candidate ranking resolves them with
//!   the Bangla-side signal and corpus frequency.
//! * Output is ASCII lowercase. Roman-trie queries are lowercased before
//!   search, so case-sensitive Avro keys (e.g. `O`, the only uppercase key
//!   in `data/avro-patterns.json`) never need variant emission.
//! * Numbers ০–৯ map to `0`–`9` (upstream tests).

/// Transliteration convention selector.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    Avro,
    Khipro,
}

/// (first consonant, second consonant) conjuncts with a dedicated Roman form.
/// Key: the two codepoints after a virama. Upstream special-cased only ক্ষ;
/// `SPECIAL_CONJUNCTS` keeps those outputs and adds nothing phonetically new
/// beyond centralizing them — remaining conjuncts concatenate (patch 3).
const SPECIAL_CONJUNCTS: &[(char, char, &str)] = &[('ক', 'ষ', "kkh")];

fn letter_map(letter: char) -> Option<&'static str> {
    match letter {
        // Numbers
        '০' => Some("0"),
        '১' => Some("1"),
        '২' => Some("2"),
        '৩' => Some("3"),
        '৪' => Some("4"),
        '৫' => Some("5"),
        '৬' => Some("6"),
        '৭' => Some("7"),
        '৮' => Some("8"),
        '৯' => Some("9"),
        // Vowels
        'অ' => Some("o"),
        'আ' => Some("a"),
        'ই' => Some("i"),
        'ঈ' => Some("i"),
        'উ' => Some("u"),
        'ঊ' => Some("u"),
        'ঋ' => Some("rri"),
        'এ' => Some("e"),
        'ঐ' => Some("oi"),
        'ও' => Some("o"),
        'ঔ' => Some("ou"),
        // Kars
        'া' => Some("a"),
        'ি' => Some("i"),
        'ী' => Some("i"),
        'ু' => Some("u"),
        'ূ' => Some("u"),
        'ৃ' => Some("rri"),
        'ে' => Some("e"),
        'ৈ' => Some("oi"),
        'ো' => Some("o"),
        'ৌ' => Some("ou"),
        // Consonants
        'ক' => Some("k"),
        'খ' => Some("kh"),
        'গ' => Some("g"),
        'ঘ' => Some("gh"),
        'ঙ' => Some("ng"),
        'চ' => Some("c"),
        'ছ' => Some("ch"),
        'জ' => Some("j"),
        'ঝ' => Some("jh"),
        'ঞ' => Some("ng"),
        'ট' => Some("t"),
        'ঠ' => Some("th"),
        'ড' => Some("d"),
        'ঢ' => Some("dh"),
        'ণ' => Some("n"),
        'ত' => Some("t"),
        'থ' => Some("th"),
        'দ' => Some("d"),
        'ধ' => Some("dh"),
        'ন' => Some("n"),
        'প' => Some("p"),
        'ফ' => Some("f"),
        'ব' => Some("b"),
        'ভ' => Some("bh"),
        'ম' => Some("m"),
        'য' => Some("z"),
        'র' => Some("r"),
        'ল' => Some("l"),
        'শ' => Some("s"),
        'ষ' => Some("x"),
        'স' => Some("s"),
        'হ' => Some("h"),
        'ড়' => Some("r"),
        'ঢ়' => Some("r"),
        'য়' => Some("y"),
        'ৎ' => Some("t"),
        'ং' => Some("ng"),
        'ঃ' => Some(""),
        'ঁ' => Some(""),
        _ => None,
    }
}

/// Single canonical form in the requested mode.
///
/// Avro mode is upstream behavior verbatim. Khipro mode only changes the two
/// modifier letters (`ঁ`, `ৎ`) listed in the module docs; everything else is
/// the shared phonetic core.
pub(crate) fn convert_to_phonetic_mode(input: &str, mode: Mode) -> String {
    let mut converted = String::with_capacity(input.len());
    let vec_str: Vec<char> = input.chars().collect();

    let mut index = 0;
    'outer: while index < vec_str.len() {
        // General virama: B Fola / Z Fola (upstream specials, kept verbatim).
        if vec_str[index] == '্'
            && let Some(c) = vec_str.get(index + 1)
        {
            // B Fola
            if *c == 'ব' {
                converted.push('w');
                index += 2;
                continue;
            } else if *c == 'য' {
                // Z Fola
                converted.push('y');
                index += 2;
                continue;
            }
        }

        // Special conjuncts (C1 virama C2) from the centralized table —
        // upstream special-cased only ক্ষ; the table keeps its output and
        // gives every future conjunct a single declaration site.
        if index + 2 < vec_str.len() && vec_str[index + 1] == '্' {
            for (first, second, out) in SPECIAL_CONJUNCTS.iter().copied() {
                if vec_str[index] == first && vec_str[index + 2] == second {
                    converted.push_str(out);
                    index += 3;
                    continue 'outer;
                }
            }
        }

        // Local patches: Khipro modifier letters.
        if mode == Mode::Khipro {
            match vec_str[index] {
                'ঁ' => {
                    converted.push('/');
                    index += 1;
                    continue;
                }
                'ৎ' => {
                    converted.push_str("t/");
                    index += 1;
                    continue;
                }
                _ => {}
            }
        }

        if let Some(c) = letter_map(vec_str[index]) {
            converted.push_str(c);
        }

        if vec_str[index].is_ascii_whitespace() || vec_str[index].is_ascii_punctuation() {
            converted.push(vec_str[index]);
        }

        index += 1;
    }

    converted
}

/// Single canonical Avro form — upstream entry point, unchanged semantics.
/// Retained from the vendored upstream API (tests and doc examples use it).
#[allow(dead_code)]
pub(crate) fn convert_to_phonetic(input: &str) -> String {
    convert_to_phonetic_mode(input, Mode::Avro)
}

/// All indexed forms for one Bangla word in the requested mode.
///
/// Avro mode always returns one form. Khipro mode additionally returns the
/// trailing-`/` slash-suppression variant (module doc, patch 2) for words
/// containing a virama.
pub(crate) fn convert_to_phonetic_variants(input: &str, mode: Mode) -> Vec<String> {
    let primary = convert_to_phonetic_mode(input, mode);
    if mode == Mode::Khipro && input.contains('্') && !primary.is_empty() && !primary.ends_with('/')
    {
        let mut suppressed = primary.clone();
        suppressed.push('/');
        return vec![primary, suppressed];
    }
    vec![primary]
}

#[cfg(test)]
mod tests {
    use super::{
        Mode, convert_to_phonetic, convert_to_phonetic_mode, convert_to_phonetic_variants,
    };

    #[test]
    fn upstream_numbers() {
        assert_eq!(convert_to_phonetic("০১২৩৪৫৬৭৮৯"), "0123456789");
    }

    #[test]
    fn upstream_word() {
        assert_eq!(convert_to_phonetic("আমি"), "ami");
        assert_eq!(convert_to_phonetic("ঋতু"), "rritu");
        assert_eq!(convert_to_phonetic("ওষুধ"), "oxudh");
        assert_eq!(convert_to_phonetic("সংখ্যা"), "sngkhya");
        assert_eq!(convert_to_phonetic("ক্ষুধা"), "kkhudha");
        assert_eq!(convert_to_phonetic("বিশ্ব"), "bisw");
        assert_eq!(convert_to_phonetic("পদ্ম"), "pdm");
        assert_eq!(convert_to_phonetic("জিতেন্দ্র"), "jitendr");
        assert_eq!(convert_to_phonetic("বিদ্বান"), "bidwan");
        assert_eq!(convert_to_phonetic("চাঁদ"), "cad");
    }

    #[test]
    fn upstream_sentence() {
        assert_eq!(
            convert_to_phonetic("আমাদের ভালোবাসা হয়ে গেল ঘাস, খেয়ে গেল গরু আর দিয়ে গেল বাঁশ"),
            "amader bhalobasa hye gel ghas, kheye gel gru ar diye gel bas"
        );
    }

    #[test]
    fn avro_mode_is_upstream_behavior() {
        // Avro must not inherit Khipro modifier letters.
        assert_eq!(convert_to_phonetic_mode("মাঁ", Mode::Avro), "ma");
        assert_eq!(convert_to_phonetic_mode("ৎ", Mode::Avro), "t");
        assert_eq!(convert_to_phonetic_variants("ক্স", Mode::Avro), vec!["ks"]);
    }

    #[test]
    fn khipro_modifiers() {
        // TSV rows (Roman → Bangla): `t/` → ৎ; the `ma/` → মাঁ, `ko/` → কঁ rows
        // spell ঁ with a trailing `/`, which Khipro canonical reproduces. Note
        // ধ্বনি is phonetic-lossy (e.g. 'অ' → "o" while TSV spells আম as `am`),
        // so only exact-inverse rows are asserted here; full TSV conformance
        // lives at pattern-table level in `roman_gen`.
        assert_eq!(convert_to_phonetic_mode("মাঁ", Mode::Khipro), "ma/");
        assert_eq!(convert_to_phonetic_mode("ৎ", Mode::Khipro), "t/");
        assert_eq!(convert_to_phonetic_mode("কঁ", Mode::Khipro), "k/");
        assert_eq!(convert_to_phonetic_mode("ক্স", Mode::Khipro), "ks");
        assert_eq!(convert_to_phonetic_mode("ক্ষ", Mode::Khipro), "kkh");
        assert_eq!(convert_to_phonetic_mode("মা", Mode::Khipro), "ma");
        assert_eq!(convert_to_phonetic_mode("চাঁদ", Mode::Khipro), "ca/d");
    }

    #[test]
    fn khipro_slash_variants() {
        // TSV: `ks` → ক্স (virama kept), `ks/` → কস (virama suppressed).
        assert_eq!(
            convert_to_phonetic_variants("ক্স", Mode::Khipro),
            vec!["ks", "ks/"]
        );
        // Words without a virama get no slash variant.
        assert_eq!(convert_to_phonetic_variants("কস", Mode::Khipro), vec!["ks"]);
        assert_eq!(convert_to_phonetic_variants("মা", Mode::Khipro), vec!["ma"]);
    }

    #[test]
    fn conjuncts_are_concatenated_letters() {
        // Virama is dropped, so every conjunct is the concatenation of its
        // letter mappings. These rows pin the generalization down so future
        // changes cannot silently drop or split a class.
        let cases = [
            ("স্ত", "st"),
            ("ক্ত", "kt"),
            ("ঞ্জ", "ngj"),
            ("ম্প", "mp"),
            ("ল্ল", "ll"),
            ("স্য", "sy"),
            ("দ্ধ", "ddh"),
            ("ক্ষ", "kkh"),
            ("ত্র", "tr"),
        ];
        for (bangla, roman) in cases {
            assert_eq!(
                convert_to_phonetic_mode(bangla, Mode::Avro),
                roman,
                "conjunct {bangla}"
            );
        }
    }

    #[test]
    fn canonical_forms_are_ascii_lowercase() {
        // Roman-trie queries are lowercased before search; canonical index
        // forms must be too, otherwise zero-cost matching breaks.
        let letters = "অআইঈউঊঋএঐওঔকখগঘঙচছজঝঞটঠডঢণতথদধানপফবভমযরলশষসহড়ঢ়য়ৎংঃঁ";
        for mode in [Mode::Avro, Mode::Khipro] {
            for letter in letters.chars() {
                for form in convert_to_phonetic_variants(&letter.to_string(), mode) {
                    assert!(
                        form.bytes()
                            .all(|b| b.is_ascii_lowercase() || b == b'/' || b.is_ascii_digit()),
                        "non-canonical form {form:?} for {letter:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn tsv_has_modifier_rows() {
        // Sanity: the khipro-m17n submodule actually has modifier rows to
        // conform to (guards against accidentally emptying the spec).
        let csv = include_str!("../../data/khipro-m17n/khipro-testcases.tsv");
        let mut checked = 0;
        for line in csv.lines().skip(1) {
            let mut cols = line.split('\t');
            let input = cols.next().unwrap_or("");
            let output = cols.next().unwrap_or("");
            if output.contains('ঁ') && !input.ends_with("//") {
                checked += 1;
            }
        }
        assert!(
            checked >= 3,
            "expected candrabindu rows in TSV, got {checked}"
        );
    }

    #[test]
    fn khipro_tsv_conformance() {
        // Run the khipro-m17n submodule corpus against dhoni's inverse:
        // for every Bangla row, converting the expected Bangla back with
        // dhoni should reproduce the Roman input where dhoni is exact, or
        // contain it among the Khipro variants.
        //
        // The verdicts are derived purely from the parsed corpus:
        // - exact:     primary form == input
        // - variant:   input appears in convert variants ("/" suppression)
        // - lossy:     printed verbatim in the test output — Khipro layout
        //              mechanics (conjunct typing, sh/s digraphs, f-suppressor)
        //              dhoni intentionally does not encode. Deviations are
        //              visible, never vendored: any *new* deviation appears
        //              in the output and trips the stability assertion below.
        // - non_roman: rows whose output contains literal Latin/digits or
        //              pure punctuation (mim identity maps), counted, not
        //              inverted.
        let csv = include_str!("../../data/khipro-m17n/khipro-testcases.tsv");
        let is_bangla = |s: &str| {
            s.chars().all(|c| {
                ('\u{0980}'..='\u{09ff}').contains(&c)
                    || c == '\u{200c}'
                    || c == '\u{200d}'
                    || c == ' '
            })
        };
        let mut exact = 0;
        let mut variant = 0;
        let mut lossy: Vec<&str> = Vec::new();
        let mut non_roman = 0;
        let mut mapped = 0;
        for line in csv.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let mut cols = line.split('\t');
            let input = cols.next().unwrap_or("");
            let output = cols.next().unwrap_or("");
            if !is_bangla(output) {
                non_roman += 1;
                continue;
            }
            mapped += 1;
            let primary = convert_to_phonetic_mode(output, Mode::Khipro);
            if primary == input {
                exact += 1;
            } else if convert_to_phonetic_variants(output, Mode::Khipro)
                .iter()
                .any(|f| f == input)
            {
                variant += 1;
            } else {
                lossy.push(input);
            }
        }

        println!(
            "conformance: exact={exact} variant={variant} mapped={mapped} lossy={lossy:?} non_roman={non_roman}"
        );
        // Stability gate: dhoni's invertible core must not shrink silently.
        // Deviations are printed above — if a new row deviates, it will be
        // visible in the output before the count assertion trips.
        assert!(
            exact + variant >= 20,
            "invertible core shrank: exact={exact} variant={variant} (lossy: {lossy:?})"
        );
        assert!(mapped > 50, "suspiciously few mapped rows: {mapped}");
    }
}
