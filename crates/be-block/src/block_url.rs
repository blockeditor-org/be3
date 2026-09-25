use std::ops::Range;

use uuid::Uuid;

const BLOCK_URL_BASE: &str = "https://blocks.pfg.pw/";
const UUID_TEXT_BYTES: usize = 36;
pub const BLOCK_URL_BYTES: usize = BLOCK_URL_BASE.len() + UUID_TEXT_BYTES * 2 + 1;
pub const BLOCK_URL_MAX_BYTES: usize = BLOCK_URL_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockUrl {
    pub range: Range<usize>,
    pub block: Uuid,
    pub workspace_id: Uuid,
}

pub fn block_url_prefix(workspace_id: Uuid) -> String {
    format!("{BLOCK_URL_BASE}{workspace_id}/")
}

pub fn block_url(workspace_id: Uuid, id: Uuid) -> String {
    format!("{}{id}", block_url_prefix(workspace_id))
}

fn parse_uuid_at(bytes: &[u8], start: usize, end: usize) -> Option<Uuid> {
    std::str::from_utf8(bytes.get(start..end)?)
        .ok()
        .and_then(|text| Uuid::parse_str(text).ok())
}

fn parse_block_url(bytes: &[u8], start: usize) -> Option<(usize, Uuid, Uuid)> {
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

pub fn parse_block_urls(bytes: &[u8]) -> Vec<BlockUrl> {
    let prefix = BLOCK_URL_BASE.as_bytes();
    let mut urls = Vec::new();
    let mut index = 0;
    while index + BLOCK_URL_BYTES <= bytes.len() {
        if !bytes[index..].starts_with(prefix) {
            index += 1;
            continue;
        }
        let Some((end, block, workspace_id)) = parse_block_url(bytes, index + prefix.len()) else {
            index += 1;
            continue;
        };
        if bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'/'))
        {
            index += 1;
            continue;
        }
        urls.push(BlockUrl {
            range: index..end,
            block,
            workspace_id,
        });
        index = end;
    }
    urls
}
