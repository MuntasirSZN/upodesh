pub const SUFFIXES: &[&str] = &["গুলো", "দের", "কে", "ে", "ের", "ত", "টি", "খানি"];
pub fn strip_suffix(word: &str) -> Option<&str> {
    let mut best: Option<usize> = None;
    for suf in SUFFIXES {
        if word.ends_with(suf) {
            let byte_idx = word.len().saturating_sub(suf.len());
            if best.is_none_or(|prev| byte_idx > prev) {
                best = Some(byte_idx);
            }
        }
    }
    best.map(|i| &word[..i])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_plural() {
        assert_eq!(strip_suffix("মানুষগুলো"), Some("মানুষ"));
    }
}
