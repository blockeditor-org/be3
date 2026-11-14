//! How the host shows a block's name inside its own text, such as a dialog's.

/// The longest name a dialog quotes, in characters, before it ends it with an ellipsis.
const MAX_QUOTED_NAME: usize = 60;

/// A block's name as a dialog quotes it inside its own curly quotes: without control or format
/// characters (newlines, bidi overrides, zero-width marks), its whitespace collapsed, curly
/// quotes made straight so a name cannot close the dialog's quote and fake what follows, and
/// cut at `MAX_QUOTED_NAME` characters.
pub fn quoted_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter_map(|c| match c {
            c if c.is_whitespace() => Some(' '),
            c if c.is_control() || is_format(c) => None,
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' | '\u{00ab}' | '\u{00bb}' => {
                Some('"')
            }
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => Some('\''),
            c => Some(c),
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.chars().count() > MAX_QUOTED_NAME {
        true => {
            let kept: String = collapsed.chars().take(MAX_QUOTED_NAME - 1).collect();
            format!("{}\u{2026}", kept.trim_end())
        }
        false => collapsed,
    }
}

/// Unicode's format characters (general category Cf).
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}
