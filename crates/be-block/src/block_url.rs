use std::ops::Range;

use uuid::Uuid;

pub const MARKER: &str = "#block/";
const UUID_TEXT_BYTES: usize = 36;
pub const MAX_BASE_BYTES: usize = 512;
pub const BLOCK_URL_FRAGMENT_BYTES: usize = MARKER.len() + UUID_TEXT_BYTES * 2 + 1;
pub const BLOCK_URL_MAX_BYTES: usize = MAX_BASE_BYTES + BLOCK_URL_FRAGMENT_BYTES;
const SCHEMES: [&[u8]; 2] = [b"https://", b"http://"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockUrl {
    pub range: Range<usize>,
    pub block: Uuid,
    pub workspace_id: Uuid,
}

pub fn block_url(base: &str, workspace_id: Uuid, id: Uuid) -> String {
    let base = base.split('#').next().unwrap_or_default();
    format!("{base}{MARKER}{workspace_id}/{id}")
}

fn parse_uuid_at(bytes: &[u8], start: usize, end: usize) -> Option<Uuid> {
    std::str::from_utf8(bytes.get(start..end)?)
        .ok()
        .and_then(|text| Uuid::parse_str(text).ok())
}

fn parse_fragment(bytes: &[u8], start: usize) -> Option<(usize, Uuid, Uuid)> {
    let workspace_end = start + UUID_TEXT_BYTES;
    let workspace_id = parse_uuid_at(bytes, start, workspace_end)?;
    if bytes.get(workspace_end) != Some(&b'/') {
        return None;
    }
    let id_start = workspace_end + 1;
    let id_end = id_start + UUID_TEXT_BYTES;
    let id = parse_uuid_at(bytes, id_start, id_end)?;
    Some((id_end, id, workspace_id))
}

fn ends_a_url(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'(' | b')' | b'<' | b'>' | b'"' | b'\'' | b'`')
}

fn base_start(bytes: &[u8], marker: usize, floor: usize) -> usize {
    let lowest = marker.saturating_sub(MAX_BASE_BYTES).max(floor);
    let mut start = marker;
    while start > lowest && !ends_a_url(bytes[start - 1]) {
        start -= 1;
    }
    SCHEMES
        .iter()
        .filter_map(|scheme| {
            memchr::memmem::rfind(&bytes[start..marker], scheme).map(|found| start + found)
        })
        .max()
        .unwrap_or(marker)
}

pub fn parse_block_urls(bytes: &[u8]) -> Vec<BlockUrl> {
    let finder = memchr::memmem::Finder::new(MARKER.as_bytes());
    let mut urls = Vec::new();
    let mut from = 0;
    let mut floor = 0;
    while let Some(found) = finder.find(&bytes[from..]) {
        let marker = from + found;
        from = marker + 1;
        let Some((end, block, workspace_id)) = parse_fragment(bytes, marker + MARKER.len()) else {
            continue;
        };
        if bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'/'))
        {
            continue;
        }
        urls.push(BlockUrl {
            range: base_start(bytes, marker, floor)..end,
            block,
            workspace_id,
        });
        from = end;
        floor = end;
    }
    urls
}
