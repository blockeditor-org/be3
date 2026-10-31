use std::collections::HashMap;

const MAGIC: &[u8] = b"!<arch>\n";
const HEADER: usize = 60;
const NAME: usize = 16;
const SIZE: std::ops::Range<usize> = 48..58;
const SYMBOLS: &str = "/";
const SYMBOLS_64: &str = "/SYM64/";
const BSD_SYMBOLS: &str = "__.SYMDEF";

pub struct Member<'a> {
    pub start: usize,
    pub header: &'a [u8],
    pub data: &'a [u8],
}

impl Member<'_> {
    fn name(&self) -> &str {
        std::str::from_utf8(&self.header[..NAME])
            .unwrap_or_default()
            .trim_end()
    }
}

pub fn is_archive(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

pub fn members(bytes: &[u8]) -> Result<Vec<Member<'_>>, String> {
    if !is_archive(bytes) {
        return Err("not an archive".to_owned());
    }
    let mut members = Vec::new();
    let mut at = MAGIC.len();
    while at < bytes.len() {
        let header = bytes
            .get(at..at + HEADER)
            .ok_or("the archive ends in the middle of a member's header")?;
        let size: usize = std::str::from_utf8(&header[SIZE])
            .ok()
            .and_then(|size| size.trim().parse().ok())
            .ok_or("an archive member's size is not a number")?;
        let data = bytes
            .get(at + HEADER..at + HEADER + size)
            .ok_or("the archive ends in the middle of a member")?;
        members.push(Member {
            start: at,
            header,
            data,
        });
        at += HEADER + size + size % 2;
    }
    Ok(members)
}

pub fn objects<'a>(members: &'a [Member<'a>]) -> impl Iterator<Item = &'a Member<'a>> {
    members
        .iter()
        .filter(|member| crate::tls::is_object(member.data))
}

fn with_size(header: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let mut header = header.to_vec();
    let text = format!("{size:<10}");
    if text.len() != SIZE.len() {
        return Err("an archive member grew past what its header can say".to_owned());
    }
    header[SIZE].copy_from_slice(text.as_bytes());
    Ok(header)
}

fn remapped(data: &[u8], width: usize, moved: &HashMap<usize, usize>) -> Result<Vec<u8>, String> {
    let read = |at: usize| -> Result<usize, String> {
        let bytes = data
            .get(at..at + width)
            .ok_or("the archive's symbol table is cut short")?;
        Ok(bytes
            .iter()
            .fold(0usize, |value, byte| (value << 8) | usize::from(*byte)))
    };
    let count = read(0)?;
    let mut out = data.to_vec();
    for entry in 0..count {
        let at = width * (entry + 1);
        let old = read(at)?;
        let new = *moved
            .get(&old)
            .ok_or("the archive's symbol table names no member")?;
        let bytes = new.to_be_bytes();
        out[at..at + width].copy_from_slice(&bytes[bytes.len() - width..]);
    }
    Ok(out)
}

pub fn rewritten(
    bytes: &[u8],
    mut rewrite: impl FnMut(&[u8]) -> Result<Option<Vec<u8>>, String>,
) -> Result<Option<Vec<u8>>, String> {
    let members = members(bytes)?;
    let mut changed = false;
    let mut contents: Vec<Vec<u8>> = Vec::with_capacity(members.len());
    for member in &members {
        let replaced = match crate::tls::is_object(member.data) {
            true => rewrite(member.data)?,
            false => None,
        };
        changed |= replaced.is_some();
        contents.push(replaced.unwrap_or_else(|| member.data.to_vec()));
    }
    if !changed {
        return Ok(None);
    }
    if members.iter().any(|member| member.name() == BSD_SYMBOLS) {
        return Err("a BSD archive cannot be rewritten".to_owned());
    }
    let mut moved = HashMap::new();
    let mut at = MAGIC.len();
    for (member, data) in members.iter().zip(&contents) {
        moved.insert(member.start, at);
        at += HEADER + data.len() + data.len() % 2;
    }
    let mut out = MAGIC.to_vec();
    for (member, data) in members.iter().zip(contents) {
        let data = match member.name() {
            SYMBOLS => remapped(&data, 4, &moved)?,
            SYMBOLS_64 => remapped(&data, 8, &moved)?,
            _ => data,
        };
        out.extend(with_size(member.header, data.len())?);
        out.extend_from_slice(&data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    Ok(Some(out))
}
