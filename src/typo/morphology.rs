use unicode_segmentation::UnicodeSegmentation;

use super::WordEntry;
use super::normalize::normalize_bengali;

/// A productive suffix rule for dictionary expansion.
///
/// `stem_ending` is removed from a lemma and `replacement` is appended. An
/// empty `stem_ending` means a plain suffix append. Rules are explicit because
/// Bengali inflection and stem alternation should not be guessed from a short
/// hard-coded suffix list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuffixRule {
    stem_ending: String,
    replacement: String,
    min_stem_graphemes: usize,
    frequency_divisor: u64,
}

impl SuffixRule {
    /// Create a suffix rule that appends `suffix` unchanged.
    pub fn append(suffix: impl Into<String>) -> Self {
        Self {
            stem_ending: String::new(),
            replacement: normalize_bengali(&suffix.into()),
            min_stem_graphemes: 2,
            frequency_divisor: 3,
        }
    }

    /// Replace a known stem ending with `replacement`. This supports simple
    /// orthographic alternations without pretending to be a full analyzer.
    pub fn replace(stem_ending: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            stem_ending: normalize_bengali(&stem_ending.into()),
            replacement: normalize_bengali(&replacement.into()),
            min_stem_graphemes: 2,
            frequency_divisor: 3,
        }
    }

    pub fn with_min_stem_graphemes(mut self, minimum: usize) -> Self {
        self.min_stem_graphemes = minimum;
        self
    }

    /// Generated forms inherit `lemma_frequency / divisor` (minimum 1).
    /// A larger divisor gives derived forms a weaker lexical prior.
    pub fn with_frequency_divisor(mut self, divisor: u64) -> Self {
        self.frequency_divisor = divisor.max(1);
        self
    }
}

/// Small, explicit suffix transducer used to expand a lemma dictionary into
/// surface forms before indexing. It does not recursively apply rules.
#[derive(Clone, Debug, Default)]
pub struct Morphology {
    suffix_rules: Vec<SuffixRule>,
}

impl Morphology {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rule(mut self, rule: SuffixRule) -> Self {
        self.suffix_rules.push(rule);
        self
    }

    pub fn add_rule(&mut self, rule: SuffixRule) {
        self.suffix_rules.push(rule);
    }

    /// Expand each lemma once with each applicable rule. The original lemma is
    /// retained. Duplicate generated forms are merged by the index builder.
    pub fn expand(&self, entries: impl IntoIterator<Item = WordEntry>) -> Vec<WordEntry> {
        let entries: Vec<WordEntry> = entries
            .into_iter()
            .map(|entry| {
                WordEntry::new(normalize_bengali(entry.word.trim()), entry.frequency.max(1))
            })
            .filter(|entry| !entry.word.is_empty())
            .collect();

        let mut expanded = entries.clone();
        for entry in &entries {
            for rule in &self.suffix_rules {
                let stem = if rule.stem_ending.is_empty() {
                    Some(entry.word.as_str())
                } else {
                    entry.word.strip_suffix(&rule.stem_ending)
                };
                let Some(stem) = stem else {
                    continue;
                };

                let stem_len = UnicodeSegmentation::graphemes(stem, true).count();
                if stem_len < rule.min_stem_graphemes {
                    continue;
                }

                let mut surface = String::with_capacity(stem.len() + rule.replacement.len());
                surface.push_str(stem);
                surface.push_str(&rule.replacement);
                if surface == entry.word {
                    continue;
                }

                expanded.push(WordEntry::new(
                    surface,
                    (entry.frequency / rule.frequency_divisor).max(1),
                ));
            }
        }
        expanded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_suffix_and_retains_lemma() {
        let morphology =
            Morphology::new().with_rule(SuffixRule::append("গুলো").with_min_stem_graphemes(2));
        let expanded = morphology.expand([WordEntry::new("মানুষ", 90)]);
        assert!(expanded.iter().any(|entry| entry.word == "মানুষ"));
        assert!(expanded.iter().any(|entry| entry.word == "মানুষগুলো"));
    }

    #[test]
    fn can_replace_stem_ending() {
        let morphology = Morphology::new().with_rule(SuffixRule::replace("ী", "িগুলো"));
        let expanded = morphology.expand([WordEntry::new("নদী", 30)]);
        assert!(expanded.iter().any(|entry| entry.word == "নদিগুলো"));
    }
}
