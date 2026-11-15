//! Snippets: where a query's clauses match a document's text, and the stretch of it around
//! the first match that a list of hits shows.

use std::ops::Range;

use crate::{
    Clause, Query,
    tokens::{fold, split},
};

/// A stretch of a text and where the query matches inside it, as byte ranges of `text`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snippet {
    pub text: String,
    pub matches: Vec<Range<usize>>,
}

/// About this many characters of context are kept on each side of the first match.
const CONTEXT: usize = 60;

impl Query {
    /// The byte ranges of `text` that a clause matches, in order and never overlapping: a word
    /// equal to a word clause, a word starting with a prefix, or a phrase's words where they
    /// occur in a row, as one range from its first word to its last. A phrase's word alone
    /// matches nothing, so the first range is where the text first matches the query, the
    /// place a hit opens at.
    pub fn matches(&self, text: &str) -> Vec<Range<usize>> {
        // Each word's terms, as the index keeps them: a long word is several, all its range.
        let terms: Vec<(Range<usize>, String)> = be_lib_graphemes::words(text.as_bytes())
            .flat_map(|range| {
                split(fold(&text[range.clone()]))
                    .into_iter()
                    .map(move |term| (range.clone(), term))
            })
            .collect();
        let mut found = Vec::new();
        for (index, (range, term)) in terms.iter().enumerate() {
            for clause in &self.clauses {
                match clause {
                    Clause::Word(word) if word == term => found.push(range.clone()),
                    Clause::Prefix(prefix) if term.starts_with(prefix.as_str()) => {
                        found.push(range.clone())
                    }
                    Clause::Phrase(phrase)
                        if terms.len() - index >= phrase.len()
                            && terms[index..index + phrase.len()]
                                .iter()
                                .zip(phrase)
                                .all(|((_, term), word)| term == word) =>
                    {
                        found.push(range.start..terms[index + phrase.len() - 1].0.end)
                    }
                    _ => {}
                }
            }
        }
        found.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(found.len());
        for range in found {
            match merged.last_mut() {
                Some(last) if range.start < last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }

    /// The stretch of `text` around the first match, cut at word gaps where it can be and
    /// marked with an ellipsis where it was cut; the start of the text when nothing matches.
    pub fn snippet(&self, text: &str) -> Snippet {
        let matches = self.matches(text);
        let first = matches.first().map_or(0, |range| range.start);
        let start = back(text, first, CONTEXT);
        let end = forward(
            text,
            matches.first().map_or(0, |range| range.end),
            CONTEXT * 2,
        );
        let mut cut = String::new();
        if start > 0 {
            cut.push('…');
        }
        let shift = cut.len();
        cut.push_str(text[start..end].trim_end());
        let kept = start + (cut.len() - shift);
        if end < text.len() {
            cut.push('…');
        }
        let cut = cut.replace(['\n', '\r', '\t'], " ");
        Snippet {
            text: cut,
            matches: matches
                .into_iter()
                .filter(|range| range.start >= start && range.end <= kept)
                .map(|range| range.start - start + shift..range.end - start + shift)
                .collect(),
        }
    }
}

/// About `chars` characters before `at`, moved on to the start of a word.
fn back(text: &str, at: usize, chars: usize) -> usize {
    let start = text[..at]
        .char_indices()
        .rev()
        .nth(chars)
        .map_or(0, |(index, _)| index);
    if start == 0 {
        return 0;
    }
    text[start..at]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map_or(start, |(gap, c)| start + gap + c.len_utf8())
        .min(at)
}

/// About `chars` characters after `at`, moved back to the end of a word.
fn forward(text: &str, at: usize, chars: usize) -> usize {
    let end = text[at..]
        .char_indices()
        .nth(chars)
        .map_or(text.len(), |(index, _)| at + index);
    if end == text.len() {
        return end;
    }
    text[at..end]
        .rfind(char::is_whitespace)
        .map_or(end, |gap| at + gap)
}
