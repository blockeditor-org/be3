use super::entry::DesktopEntry;

const FILE_CODES: [char; 10] = ['f', 'F', 'u', 'U', 'd', 'D', 'n', 'N', 'v', 'm'];

#[derive(Clone, Debug, PartialEq)]
struct Token {
    text: String,
    quoted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError {
    Unterminated,
    Empty,
    NoTerminal,
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unterminated => "its Exec line has an unterminated quote",
            Self::Empty => "its Exec line names no program",
            Self::NoTerminal => "it runs in a terminal and no terminal emulator was found",
        })
    }
}

fn tokens(exec: &str) -> Result<Vec<Token>, ExecError> {
    let mut out = Vec::new();
    let mut chars = exec.chars().peekable();
    loop {
        while chars
            .peek()
            .is_some_and(|next| *next == ' ' || *next == '\t')
        {
            chars.next();
        }
        if chars.peek().is_none() {
            return Ok(out);
        }
        let mut token = Token {
            text: String::new(),
            quoted: false,
        };
        while let Some(&next) = chars.peek() {
            match next {
                ' ' | '\t' => break,
                '"' => {
                    chars.next();
                    token.quoted = true;
                    loop {
                        match chars.next() {
                            None => return Err(ExecError::Unterminated),
                            Some('"') => break,
                            Some('\\') => match chars.next() {
                                Some(escaped @ ('"' | '`' | '$' | '\\')) => {
                                    token.text.push(escaped)
                                }
                                Some(other) => {
                                    token.text.push('\\');
                                    token.text.push(other);
                                }
                                None => return Err(ExecError::Unterminated),
                            },
                            Some(other) => token.text.push(other),
                        }
                    }
                }
                '\\' => {
                    chars.next();
                    if let Some(escaped) = chars.next() {
                        token.text.push(escaped);
                    }
                }
                other => {
                    chars.next();
                    token.text.push(other);
                }
            }
        }
        out.push(token);
    }
}

fn expand(text: &str, entry: &DesktopEntry) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(next) = chars.next() {
        if next != '%' {
            out.push(next);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('c') => out.push_str(&entry.name),
            Some('k') => out.push_str(&entry.path),
            Some('i') => {
                if let Some(icon) = &entry.icon {
                    out.push_str(icon);
                }
            }
            Some(_) | None => {}
        }
    }
    out
}

pub fn arguments(entry: &DesktopEntry) -> Result<Vec<String>, ExecError> {
    let mut out = Vec::new();
    for token in tokens(&entry.exec)? {
        if !token.quoted {
            let mut code = token.text.strip_prefix('%').map(|rest| rest.chars());
            let single = code
                .as_mut()
                .and_then(|chars| Some((chars.next()?, chars.next().is_none())));
            match single {
                Some((code, true)) if FILE_CODES.contains(&code) => continue,
                Some(('i', true)) => {
                    if let Some(icon) = &entry.icon {
                        out.push("--icon".to_owned());
                        out.push(icon.clone());
                    }
                    continue;
                }
                _ => {}
            }
        }
        out.push(expand(&token.text, entry));
    }
    match out.is_empty() {
        true => Err(ExecError::Empty),
        false => Ok(out),
    }
}

pub fn command(
    entry: &DesktopEntry,
    terminal: Option<&[String]>,
) -> Result<Vec<String>, ExecError> {
    let arguments = arguments(entry)?;
    if !entry.terminal {
        return Ok(arguments);
    }
    let terminal = terminal.ok_or(ExecError::NoTerminal)?;
    Ok(terminal.iter().cloned().chain(arguments).collect())
}
