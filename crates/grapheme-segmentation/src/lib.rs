/* @yeskeep
Extended grapheme clusters (UAX #29, https://www.unicode.org/reports/tr29/) at
the Unicode version of icu_properties. This is the only grapheme segmenter:
text-editor-core's cursor stops and beui-core's galley cursors both use it.
It also holds the one word rule: text-editor-core's word stops and double
click, and beui-view's selectable text, all use word_classes and word_around.

A query is one byte offset, inside a scalar or not, and malformed UTF-8 is one
U+FFFD per byte, as text-editor-core edits raw bytes. It reads only as far back
as GB9c, GB11 and GB12/13 need, which can be unbounded: don't cap the lookbehind
or segment a whole line per query. unicode-segmentation's GraphemeCursor is no
substitute; its chunked pre-context API is wrong for flags and emoji.

To upgrade Unicode, bump icu_properties, replace tests/GraphemeBreakTest.txt
with that version's ucd/auxiliary/GraphemeBreakTest.txt, and apply the rule
changes listed in UAX #29's Modifications section.
*/

use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{ExtendedPictographic, GraphemeClusterBreak as Gcb, IndicConjunctBreak as Incb},
};

fn gcb(c: char) -> Gcb {
    CodePointMapData::<Gcb>::new().get(c)
}

fn incb(c: char) -> Incb {
    CodePointMapData::<Incb>::new().get(c)
}

fn pictographic(c: char) -> bool {
    CodePointSetData::new::<ExtendedPictographic>().contains(c)
}

fn decode(bytes: &[u8], index: usize) -> (char, usize) {
    let width = match bytes[index] {
        0..=0x7f => return (bytes[index] as char, 1),
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return ('\u{fffd}', 1),
    };
    if let Some(slice) = bytes.get(index..index + width)
        && let Ok(text) = std::str::from_utf8(slice)
    {
        return (text.chars().next().unwrap(), width);
    }
    ('\u{fffd}', 1)
}

fn previous(bytes: &[u8], index: &mut usize) -> Option<char> {
    if *index == 0 {
        return None;
    }
    for start in (index.saturating_sub(4)..*index).rev() {
        let (c, width) = decode(bytes, start);
        if start + width == *index {
            if width == 1 && !bytes[start].is_ascii() {
                continue;
            }
            *index = start;
            return Some(c);
        }
    }
    *index -= 1;
    Some('\u{fffd}')
}

pub fn is_grapheme_boundary(bytes: &[u8], index: usize) -> bool {
    assert!(index <= bytes.len());
    // GB1, GB2: break at the start and end of text. @yeskeep
    if index == 0 || index == bytes.len() {
        return true;
    }
    // Between two ASCII bytes only GB3 can apply. @yeskeep
    if bytes[index - 1].is_ascii() && bytes[index].is_ascii() {
        return bytes[index - 1] != b'\r' || bytes[index] != b'\n';
    }
    if bytes[index] & 0xc0 == 0x80 {
        for start in index.saturating_sub(3)..index {
            if start + decode(bytes, start).1 > index {
                return false;
            }
        }
    }
    let mut before = index;
    let left = previous(bytes, &mut before).unwrap();
    let right = decode(bytes, index).0;
    let (l, r) = (gcb(left), gcb(right));
    // GB3: CR × LF @yeskeep
    if l == Gcb::CR && r == Gcb::LF {
        return false;
    }
    // GB4: (Control | CR | LF) ÷, GB5: ÷ (Control | CR | LF) @yeskeep
    if matches!(l, Gcb::Control | Gcb::CR | Gcb::LF)
        || matches!(r, Gcb::Control | Gcb::CR | Gcb::LF)
    {
        return true;
    }
    // GB6: L × (L | V | LV | LVT) @yeskeep
    // GB7: (LV | V) × (V | T) @yeskeep
    // GB8: (LVT | T) × T @yeskeep
    // GB9: × (Extend | ZWJ) @yeskeep
    // GB9a: × SpacingMark @yeskeep
    // GB9b: Prepend × @yeskeep
    if l == Gcb::L && matches!(r, Gcb::L | Gcb::V | Gcb::LV | Gcb::LVT)
        || matches!(l, Gcb::LV | Gcb::V) && matches!(r, Gcb::V | Gcb::T)
        || matches!(l, Gcb::LVT | Gcb::T) && r == Gcb::T
        || matches!(r, Gcb::Extend | Gcb::ZWJ | Gcb::SpacingMark)
        || l == Gcb::Prepend
    {
        return false;
    }
    // GB9c: \p{InCB=Linker} \p{InCB=Extend}* × \p{InCB=Consonant} @yeskeep
    if incb(right) == Incb::Consonant {
        let mut cursor = index;
        while let Some(c) = previous(bytes, &mut cursor) {
            match incb(c) {
                Incb::Extend => {}
                Incb::Linker => return false,
                _ => break,
            }
        }
    }
    // GB11: \p{Extended_Pictographic} Extend* ZWJ × \p{Extended_Pictographic} @yeskeep
    if l == Gcb::ZWJ && pictographic(right) {
        let mut cursor = before;
        while let Some(c) = previous(bytes, &mut cursor) {
            if gcb(c) == Gcb::Extend {
                continue;
            }
            if pictographic(c) {
                return false;
            }
            break;
        }
    }
    // GB12: sot (RI RI)* RI × RI, GB13: [^RI] (RI RI)* RI × RI @yeskeep
    if l == Gcb::RegionalIndicator && r == Gcb::RegionalIndicator {
        let mut cursor = before;
        let mut odd = true;
        while let Some(c) = previous(bytes, &mut cursor) {
            if gcb(c) != Gcb::RegionalIndicator {
                break;
            }
            odd = !odd;
        }
        return !odd;
    }
    // GB999: Any ÷ Any @yeskeep
    true
}

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordClass {
    Space,
    Word,
    Symbol,
}

fn word_class(c: char) -> WordClass {
    if c.is_whitespace() {
        WordClass::Space
    } else if c.is_alphanumeric() || c == '_' {
        WordClass::Word
    } else {
        WordClass::Symbol
    }
}

fn cluster_before(bytes: &[u8], index: usize) -> Option<(char, usize)> {
    let mut start = index;
    previous(bytes, &mut start)?;
    while !is_grapheme_boundary(bytes, start) {
        previous(bytes, &mut start);
    }
    Some((decode(bytes, start).0, start))
}

fn cluster_after(bytes: &[u8], index: usize) -> Option<(char, usize)> {
    if index >= bytes.len() {
        return None;
    }
    let (c, width) = decode(bytes, index);
    let mut end = index + width;
    while !is_grapheme_boundary(bytes, end) {
        end += decode(bytes, end).1;
    }
    Some((c, end))
}

pub fn word_classes(bytes: &[u8], index: usize) -> Option<(WordClass, WordClass)> {
    if !is_grapheme_boundary(bytes, index) {
        return None;
    }
    let (before, _) = cluster_before(bytes, index)?;
    let (after, _) = cluster_after(bytes, index)?;
    Some((word_class(before), word_class(after)))
}

pub fn word_around(bytes: &[u8], index: usize, joined: impl Fn(usize) -> bool) -> (usize, usize) {
    let mut index = index.min(bytes.len());
    while !is_grapheme_boundary(bytes, index) {
        index -= 1;
    }
    let in_line = |c: char| !matches!(c, '\n' | '\r');
    let after = cluster_after(bytes, index)
        .map(|(c, _)| c)
        .filter(|c| in_line(*c));
    let before = cluster_before(bytes, index)
        .map(|(c, _)| c)
        .filter(|c| in_line(*c));
    let class = match (after.map(word_class), before.map(word_class)) {
        (Some(class), _) | (_, Some(class)) if class != WordClass::Space => class,
        (None, None) => return (index, index),
        _ => WordClass::Space,
    };
    let same = |c: char| in_line(c) && word_class(c) == class;
    let mut start = index;
    while let Some((c, previous)) = cluster_before(bytes, start)
        && (same(c) || joined(start))
    {
        start = previous;
    }
    let mut end = index;
    while let Some((c, next)) = cluster_after(bytes, end)
        && (same(c) || joined(end))
    {
        end = next;
    }
    (start, end)
}
