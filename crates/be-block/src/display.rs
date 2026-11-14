//! Block names as the host's own words show them: inside a dialog's sentence, where a name must
//! not be able to fake the words around it.

use icu_properties::{CodePointMapData, props::GeneralCategory};

/// The longest name a dialog quotes, in characters, before it ends it with an ellipsis.
const MAX_QUOTED_NAME: usize = 60;

/// The most combining marks a quoted name keeps on one character: enough for the scripts that
/// stack two (Vietnamese, Thai), too few to stack marks over the dialog's own words.
const MAX_COMBINING: usize = 2;

/// A block's name made fit to quote inside a dialog's words. The name loses control and format
/// characters (newlines, bidi overrides and isolates, zero-width spaces; a zero-width joiner or
/// non-joiner stays only between two letters) and any combining mark with no letter under it;
/// blank-looking letters
/// become spaces, whitespace collapses, every quotation mark and lookalike is made straight so a
/// name cannot close the dialog's quote and fake what follows, runs of combining marks are cut to
/// `MAX_COMBINING`, and it is cut at `MAX_QUOTED_NAME` characters on a grapheme boundary. The
/// dialog quotes it and isolates it for bidi through its text (beui's `Phrase::quoted`), not with
/// characters in the string.
pub fn quotable_name(name: &str) -> String {
    let categories = CodePointMapData::<GeneralCategory>::new();
    let mut cleaned = String::with_capacity(name.len());
    // Marks on the last kept character: what is dropped between marks does not end their run. It
    // starts full, and a space fills it, so a mark with no kept letter under it goes.
    let mut combining = MAX_COMBINING;
    // A zero-width joiner or non-joiner waits for the next kept letter: it stays only between two
    // letters (an emoji sequence, a Persian or Indic spelling), where it can neither hide nor
    // move a word.
    let mut joiner = None;
    for c in name.chars() {
        let category = categories.get(c);
        let kept = match category {
            GeneralCategory::NonspacingMark | GeneralCategory::EnclosingMark => {
                combining += 1;
                if combining <= MAX_COMBINING {
                    cleaned.push(c);
                }
                continue;
            }
            _ if c.is_whitespace() || is_blank_letter(c) => ' ',
            _ if matches!(c, '\u{200c}' | '\u{200d}') => {
                if cleaned.chars().next_back().is_some_and(|last| last != ' ') {
                    joiner = Some(c);
                }
                continue;
            }
            GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator => continue,
            _ => straight_quote(c).unwrap_or(c),
        };
        match (kept, joiner.take()) {
            (' ', _) => combining = MAX_COMBINING,
            (_, Some(joiner)) => {
                cleaned.push(joiner);
                combining = 0;
            }
            (_, None) => combining = 0,
        }
        cleaned.push(kept);
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some((cut, _)) = collapsed.char_indices().nth(MAX_QUOTED_NAME - 1) else {
        return collapsed;
    };
    if collapsed[cut..].chars().count() == 1 {
        return collapsed;
    }
    let bytes = collapsed.as_bytes();
    let cut = (0..=cut)
        .rev()
        .find(|at| {
            collapsed.is_char_boundary(*at) && be_lib_graphemes::is_grapheme_boundary(bytes, *at)
        })
        .unwrap_or(0);
    format!("{}\u{2026}", collapsed[..cut].trim_end())
}

/// Letters that draw as nothing (Hangul fillers, the blank Braille pattern), which a name could
/// use to look like spaces that collapsing does not see.
fn is_blank_letter(c: char) -> bool {
    matches!(
        c,
        '\u{115f}' | '\u{1160}' | '\u{2800}' | '\u{3164}' | '\u{ffa0}'
    )
}

/// The straight quote a quotation mark (Unicode's Quotation_Mark property) or a lookalike reads
/// as.
fn straight_quote(c: char) -> Option<char> {
    match c {
        '\u{00ab}'
        | '\u{00bb}'
        | '\u{02ba}'
        | '\u{02ee}'
        | '\u{05f4}'
        | '\u{201c}'..='\u{201f}'
        | '\u{2033}'
        | '\u{2036}'
        | '\u{2e42}'
        | '\u{3003}'
        | '\u{300e}'
        | '\u{300f}'
        | '\u{301d}'..='\u{301f}'
        | '\u{275d}'
        | '\u{275e}'
        | '\u{275f}'
        | '\u{2760}'
        | '\u{fe43}'
        | '\u{fe44}'
        | '\u{ff02}'
        | '\u{1f676}'..='\u{1f678}' => Some('"'),
        '\u{02bb}'
        | '\u{02bc}'
        | '\u{055a}'
        | '\u{05f3}'
        | '\u{2018}'..='\u{201b}'
        | '\u{2032}'
        | '\u{2035}'
        | '\u{2039}'
        | '\u{203a}'
        | '\u{275b}'
        | '\u{275c}'
        | '\u{300c}'
        | '\u{300d}'
        | '\u{fe41}'
        | '\u{fe42}'
        | '\u{ff07}'
        | '\u{ff62}'
        | '\u{ff63}' => Some('\''),
        _ => None,
    }
}
