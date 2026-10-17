mod tables;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gcb {
    Other,
    CR,
    LF,
    Control,
    Extend,
    Zwj,
    RegionalIndicator,
    Prepend,
    SpacingMark,
    L,
    V,
    T,
    LV,
    Lvt,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Incb {
    None,
    Consonant,
    Extend,
    Linker,
}

fn property<T: Copy>(c: char, table: &[(u32, u32, T)], default: T) -> T {
    let c = c as u32;
    let i = table.partition_point(|&(_, end, _)| end < c);
    table
        .get(i)
        .filter(|&&(start, _, _)| start <= c)
        .map_or(default, |&(_, _, value)| value)
}

fn gcb(c: char) -> Gcb {
    property(c, tables::GCB, Gcb::Other)
}
fn incb(c: char) -> Incb {
    property(c, tables::INCB, Incb::None)
}
fn pictographic(c: char) -> bool {
    property(c, tables::EXTENDED_PICTOGRAPHIC, false)
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
    if index == 0 || index == bytes.len() {
        return true;
    }
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
    use Gcb::*;
    if l == CR && r == LF {
        return false;
    }
    if matches!(l, Control | CR | LF) || matches!(r, Control | CR | LF) {
        return true;
    }
    if l == L && matches!(r, L | V | LV | Lvt)
        || matches!(l, LV | V) && matches!(r, V | T)
        || matches!(l, Lvt | T) && r == T
        || matches!(r, Extend | Zwj | SpacingMark)
        || l == Prepend
    {
        return false;
    }
    if incb(right) == Incb::Consonant {
        let mut cursor = index;
        let mut linker = false;
        while let Some(c) = previous(bytes, &mut cursor) {
            match incb(c) {
                Incb::Extend => {}
                Incb::Linker => linker = true,
                Incb::Consonant if linker => return false,
                _ => break,
            }
        }
    }
    if l == Zwj && pictographic(right) {
        let mut cursor = before;
        while let Some(c) = previous(bytes, &mut cursor) {
            if gcb(c) == Extend {
                continue;
            }
            if pictographic(c) {
                return false;
            }
            break;
        }
    }
    if l == RegionalIndicator && r == RegionalIndicator {
        let mut cursor = before;
        let mut odd = true;
        while let Some(c) = previous(bytes, &mut cursor) {
            if gcb(c) != RegionalIndicator {
                break;
            }
            odd = !odd;
        }
        return !odd;
    }
    true
}

#[cfg(test)]
mod tests;
