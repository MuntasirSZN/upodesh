use std::borrow::Cow;

use unicode_normalization::{UnicodeNormalization, is_nfc};

/// Canonical Unicode normalization shared by indexing and lookup.
///
/// Do not strip Bengali marks, virama, ZWJ or ZWNJ here: those can change the
/// spelling/orthography. NFC only makes canonically equivalent encodings equal.
pub(crate) fn normalize_bengali(input: &str) -> String {
    input.nfc().collect()
}

/// Borrow the input when it is already NFC, avoiding an allocation on the
/// common path. The caller must treat the returned `Cow` as the normalized
/// text; when normalization is needed the owned string is NFC.
pub(crate) fn normalize_cow(input: &str) -> Cow<'_, str> {
    if is_nfc(input) {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(input.nfc().collect())
    }
}
