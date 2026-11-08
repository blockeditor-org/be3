//! Splitting text into terms: be-lib-graphemes' words, case folded.

use crate::{BODY, NAME, Text};

/// Longer words are not indexed (hashes, base64, run-together noise).
const LONGEST_TERM: usize = 64;

/// The folded form of a word, as terms are kept.
pub fn fold(word: &str) -> String {
    caseless::default_case_fold_str(word)
}

/// The terms of `text` in order, folded.
pub fn terms(text: &str) -> impl Iterator<Item = String> + '_ {
    be_lib_graphemes::words(text.as_bytes())
        .map(|range| fold(&text[range]))
        .filter(|term| term.len() <= LONGEST_TERM)
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
