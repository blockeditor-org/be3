use super::SynHlColorScope;

pub(super) fn scopes(bytes: &[u8]) -> Vec<SynHlColorScope> {
    use SynHlColorScope as Scope;

    let mut scopes = vec![Scope::Unstyled; bytes.len()];
    let mut index = 0;
    let mut line_start = true;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\n' {
            line_start = true;
            index += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let at_line_start = std::mem::take(&mut line_start);
        if byte == b'#' {
            let end = line_end(bytes, index);
            scopes[index..end].fill(Scope::Comment);
            index = end;
            continue;
        }
        if byte == b'[' && at_line_start {
            index = header(bytes, index, &mut scopes);
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            let end = string(bytes, index, &mut scopes);
            if followed_by_equals(bytes, end) {
                scopes[index..end].fill(Scope::Variable);
            }
            index = end;
            continue;
        }
        if is_bare(byte) {
            let end = bare_end(bytes, index);
            let token = &bytes[index..end];
            let scope = if followed_by_equals(bytes, end) {
                Scope::Variable
            } else if matches!(token, b"true" | b"false" | b"inf" | b"nan")
                || token[0].is_ascii_digit()
                || matches!(token[0], b'+' | b'-')
            {
                Scope::Literal
            } else {
                Scope::Invalid
            };
            scopes[index..end].fill(scope);
            index = end;
            continue;
        }
        scopes[index] = match byte {
            b'=' => Scope::PunctuationImportant,
            b'[' | b']' | b'{' | b'}' | b',' | b'.' => Scope::Punctuation,
            _ => Scope::Invalid,
        };
        index += 1;
    }
    scopes
}

fn is_bare(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+')
}

fn bare_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && (is_bare(bytes[index]) || matches!(bytes[index], b':' | b'.')) {
        if bytes[index] == b'.' && !bytes[..index].last().is_some_and(u8::is_ascii_digit) {
            break;
        }
        index += 1;
    }
    index
}

fn line_end(bytes: &[u8], index: usize) -> usize {
    bytes[index..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| index + offset)
}

fn followed_by_equals(bytes: &[u8], mut index: usize) -> bool {
    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
        index += 1;
    }
    matches!(bytes.get(index), Some(b'=' | b'.'))
}

fn header(bytes: &[u8], start: usize, scopes: &mut [SynHlColorScope]) -> usize {
    use SynHlColorScope as Scope;

    let end = line_end(bytes, start);
    let mut index = start;
    while index < end {
        match bytes[index] {
            b'#' => {
                scopes[index..end].fill(Scope::Comment);
                return end;
            }
            b'"' | b'\'' => {
                let after = string(bytes, index, scopes);
                scopes[index..after].fill(Scope::KeywordStorage);
                index = after;
            }
            b'[' | b']' | b'.' => {
                scopes[index] = Scope::Punctuation;
                index += 1;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            _ => {
                scopes[index] = Scope::KeywordStorage;
                index += 1;
            }
        }
    }
    end
}

fn string(bytes: &[u8], start: usize, scopes: &mut [SynHlColorScope]) -> usize {
    use SynHlColorScope as Scope;

    let quote = bytes[start];
    let triple = bytes[start..].starts_with(&[quote; 3]);
    let open = if triple { 3 } else { 1 };
    scopes[start..start + open].fill(Scope::Punctuation);
    let mut index = start + open;
    while index < bytes.len() {
        if !triple && bytes[index] == b'\n' {
            return index;
        }
        if bytes[index..].starts_with(&[quote; 3][..open]) {
            scopes[index..index + open].fill(Scope::Punctuation);
            return index + open;
        }
        if quote == b'"' && bytes[index] == b'\\' {
            let escaped = (index + 2).min(bytes.len());
            scopes[index..escaped].fill(Scope::Literal);
            index = escaped;
            continue;
        }
        scopes[index] = Scope::LiteralString;
        index += 1;
    }
    index
}
