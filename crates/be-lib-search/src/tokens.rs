//! Splitting text into terms: be-lib-graphemes' words, NFKC normalized and
//! case folded. Until be-lib-graphemes has UAX #29 words, a run of CJK or
//! Thai is one word, found only by its start (a prefix) or whole.

use unicode_normalization::UnicodeNormalization;

use crate::{BODY, NAME, Text};

/// A longer word is split into terms of at most this many bytes.
const LONGEST_TERM: usize = 64;

/// The folded form of a word, as terms are kept.
pub fn fold(word: &str) -> String {
    let folded = caseless::default_case_fold_str(&word.nfkc().collect::<String>());
    folded.nfc().collect()
}

/// The terms of `text` in order, folded.
pub fn terms(text: &str) -> impl Iterator<Item = String> + '_ {
    be_lib_graphemes::words(text.as_bytes()).flat_map(|range| split(fold(&text[range])))
}

/// A term cut into pieces of at most `LONGEST_TERM` bytes on character
/// boundaries.
fn split(term: String) -> Vec<String> {
    if term.len() <= LONGEST_TERM {
        return vec![term];
    }
    let mut pieces = Vec::new();
    let mut rest = term.as_str();
    while !rest.is_empty() {
        let mut end = rest.len().min(LONGEST_TERM);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        pieces.push(rest[..end].to_owned());
        rest = &rest[end..];
    }
    pieces
}

/// A term as the dictionary keys it: its field's byte, then the term.
pub(crate) fn key(field: u8, term: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(term.len() + 1);
    key.push(field);
    key.extend_from_slice(term.as_bytes());
    key
}

/// The keys of a document's terms with their positions, per field, and each
/// field's length in terms.
pub(crate) fn document_terms(text: &Text) -> (Vec<(Vec<u8>, u32)>, [u32; 2]) {
    let mut out = Vec::new();
    let mut lengths = [0; 2];
    for (field, text) in [(NAME, &text.name), (BODY, &text.body)] {
        for (position, term) in terms(text).enumerate() {
            out.push((key(field, &term), position as u32));
            lengths[field as usize] = position as u32 + 1;
        }
    }
    (out, lengths)
}
