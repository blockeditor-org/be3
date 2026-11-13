//! Just enough HTML for the clipboard: a tokenizer that takes any input,
//! however malformed, in one pass without recursion, and escaping for writing
//! it. A schema maps tags to its marks (`schema::basic::html`).

/// A piece of HTML. Names are lowercase; text has its entities decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Open {
        name: String,
        attributes: Vec<(String, String)>,
        closed: bool,
    },
    Close {
        name: String,
    },
    Text(String),
}

impl Token {
    pub fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
        attributes
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.as_str())
    }
}

/// The tokens of `html`, between `<!--StartFragment-->` and
/// `<!--EndFragment-->` when a browser marked them. Comments, declarations and
/// the contents of `script`, `style`, `head` and `title` are dropped.
pub fn tokens(html: &str) -> Vec<Token> {
    let html = fragment(html);
    let mut tokens = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            tokens.push(Token::Text(decode(rest)));
            break;
        };
        if open > 0 {
            tokens.push(Token::Text(decode(&rest[..open])));
        }
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
            continue;
        }
        let Some((token, after)) = tag(rest) else {
            tokens.push(Token::Text("<".to_owned()));
            rest = &rest[1..];
            continue;
        };
        rest = after;
        if let Token::Open { name, closed: false, .. } = &token
            && matches!(name.as_str(), "script" | "style" | "head" | "title")
        {
            let close = format!("</{name}");
            rest = find_ignoring_case(rest, &close)
                .and_then(|at| rest[at..].find('>').map(|end| &rest[at + end + 1..]))
                .unwrap_or("");
            continue;
        }
        tokens.push(token);
    }
    tokens
}

fn fragment(html: &str) -> &str {
    const START: &str = "<!--StartFragment-->";
    const END: &str = "<!--EndFragment-->";
    let Some(start) = html.find(START) else {
        return html;
    };
    let inner = &html[start + START.len()..];
    inner.find(END).map_or(inner, |end| &inner[..end])
}

fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// A tag at the start of `text`, which starts with `<`, and what follows it;
/// `None` when it is no tag, as in `a < b`.
fn tag(text: &str) -> Option<(Token, &str)> {
    let bytes = text.as_bytes();
    let mut at = 1;
    let closing = bytes.get(at) == Some(&b'/');
    if closing {
        at += 1;
    }
    let name_start = at;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-') {
        at += 1;
    }
    if at == name_start || !bytes[name_start].is_ascii_alphabetic() {
        return None;
    }
    let name = text[name_start..at].to_ascii_lowercase();
    let mut attributes = Vec::new();
    let mut closed = false;
    loop {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        match bytes.get(at) {
            None => return Some((open_or_close(closing, name, attributes, closed), "")),
            Some(b'>') => {
                let token = open_or_close(closing, name, attributes, closed);
                return Some((token, &text[at + 1..]));
            }
            Some(b'/') => {
                closed = true;
                at += 1;
                continue;
            }
            _ => {}
        }
        closed = false;
        let key_start = at;
        while at < bytes.len()
            && !bytes[at].is_ascii_whitespace()
            && !matches!(bytes[at], b'=' | b'>' | b'/')
        {
            at += 1;
        }
        if at == key_start {
            at += 1;
            continue;
        }
        let key = text[key_start..at].to_ascii_lowercase();
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let mut value = String::new();
        if bytes.get(at) == Some(&b'=') {
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            match bytes.get(at) {
                Some(quote @ (b'"' | b'\'')) => {
                    let end = text[at + 1..]
                        .find(char::from(*quote))
                        .map_or(text.len(), |end| at + 1 + end);
                    value = decode(&text[at + 1..end]);
                    at = (end + 1).min(text.len());
                }
                _ => {
                    let start = at;
                    while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'>'
                    {
                        at += 1;
                    }
                    value = decode(&text[start..at]);
                }
            }
        }
        attributes.push((key, value));
    }
}

fn open_or_close(
    closing: bool,
    name: String,
    attributes: Vec<(String, String)>,
    closed: bool,
) -> Token {
    match closing {
        true => Token::Close { name },
        false => Token::Open {
            name,
            attributes,
            closed,
        },
    }
}

/// `text` with its character references decoded; one it does not know stays
/// as it is.
pub fn decode(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let reference = rest[1..]
            .bytes()
            .take(12)
            .position(|byte| byte == b';')
            .and_then(|end| Some((character(&rest[1..1 + end])?, end + 2)));
        match reference {
            Some((letter, len)) => {
                decoded.push(letter);
                rest = &rest[len..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

fn character(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).filter(|letter| *letter != '\0');
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => return None,
    })
}

/// `text` escaped for an element's text or a quoted attribute.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for letter in text.chars() {
        match letter {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(letter),
        }
    }
    escaped
}
