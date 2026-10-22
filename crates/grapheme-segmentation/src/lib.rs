/* @yeskeep
Extended grapheme clusters (UAX #29, https://www.unicode.org/reports/tr29/) at
the Unicode version of icu_properties. This is the only grapheme segmenter:
text-editor-core's cursor stops and beui-core's galley cursors both use it.

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
