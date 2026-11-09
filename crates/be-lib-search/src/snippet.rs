//! Snippets: where a query's clauses match a document's text, and the stretch of it around
//! the first match that a list of hits shows.

use std::ops::Range;

use crate::{Clause, Query, tokens::fold};

/// A stretch of a text and where the query matches inside it, as byte ranges of `text`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snippet {
    pub text: String,
    pub matches: Vec<Range<usize>>,
}

/// About this many characters of context are kept on each side of the first match.
const CONTEXT: usize = 60;

impl Query {
    /// The byte ranges of `text`'s words that a clause matches: a word or a phrase's word
    /// equal to one of the clauses, or a word starting with a prefix.
    pub fn matches(&self, text: &str) -> Vec<Range<usize>> {
        let mut words = Vec::new();
        let mut prefixes = Vec::new();
        for clause in &self.clauses {
            match clause {
                Clause::Word(word) => words.push(word.as_str()),
                Clause::Prefix(prefix) => prefixes.push(prefix.as_str()),
                Clause::Phrase(phrase) => words.extend(phrase.iter().map(String::as_str)),
            }
        }
        be_lib_graphemes::words(text.as_bytes())
            .filter(|range| {
                let folded = fold(&text[range.clone()]);
                words.contains(&folded.as_str())
                    || prefixes.iter().any(|prefix| folded.starts_with(prefix))
            })
            .collect()
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
        .find(char::is_whitespace)
        .map_or(start, |gap| start + gap + 1)
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
