use std::ops::Range;

use uuid::Uuid;

use crate::BlockRef;

const BLOCK_URL_BASE: &str = "https://blocks.pfg.pw/";
const REPO_RELATIVE_BLOCK_URL_MARKER: &str = "repo/";
const UUID_TEXT_BYTES: usize = 36;
pub const BLOCK_URL_BYTES: usize = BLOCK_URL_BASE.len() + UUID_TEXT_BYTES * 2 + 1;
pub const BLOCK_URL_MAX_BYTES: usize =
    BLOCK_URL_BASE.len() + REPO_RELATIVE_BLOCK_URL_MARKER.len() + UUID_TEXT_BYTES * 2 + 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockUrl {
    pub range: Range<usize>,
    pub reference: BlockRef,
    pub workspace_id: Option<Uuid>,
}

pub fn block_url_prefix(workspace_id: Uuid) -> String {
    format!("{BLOCK_URL_BASE}{workspace_id}/")
}

pub fn block_url(workspace_id: Uuid, id: Uuid) -> String {
    format!("{}{id}", block_url_prefix(workspace_id))
}

pub fn repo_relative_block_url(repo: Uuid, eternal_id: Uuid) -> String {
    format!("{BLOCK_URL_BASE}{REPO_RELATIVE_BLOCK_URL_MARKER}{repo}/{eternal_id}")
}

pub fn block_ref_url(workspace_id: Uuid, reference: &BlockRef) -> String {
    match reference {
        BlockRef::Direct(id) => block_url(workspace_id, *id),
        BlockRef::RepoRelative { repo, eternal_id } => repo_relative_block_url(*repo, *eternal_id),
    }
}

fn parse_uuid_at(bytes: &[u8], start: usize, end: usize) -> Option<Uuid> {
    std::str::from_utf8(bytes.get(start..end)?)
        .ok()
        .and_then(|text| Uuid::parse_str(text).ok())
}

struct ParsedBlockUrl {
    end: usize,
    reference: BlockRef,
    workspace_id: Option<Uuid>,
}

fn parse_direct_block_url(bytes: &[u8], start: usize) -> Option<ParsedBlockUrl> {
    let workspace_end = start + UUID_TEXT_BYTES;
    let workspace_id = parse_uuid_at(bytes, start, workspace_end)?;
    if bytes.get(workspace_end) != Some(&b'/') {
        return None;
    }
    let id_start = workspace_end + 1;
    let id_end = id_start + UUID_TEXT_BYTES;
    let id = parse_uuid_at(bytes, id_start, id_end)?;
    Some(ParsedBlockUrl {
        end: id_end,
        reference: BlockRef::Direct(id),
        workspace_id: Some(workspace_id),
    })
}

fn parse_repo_relative_block_url(bytes: &[u8], start: usize) -> Option<ParsedBlockUrl> {
    let marker_end = start + REPO_RELATIVE_BLOCK_URL_MARKER.len();
    if bytes.get(start..marker_end) != Some(REPO_RELATIVE_BLOCK_URL_MARKER.as_bytes()) {
        return None;
    }
    let repo_end = marker_end + UUID_TEXT_BYTES;
    let repo = parse_uuid_at(bytes, marker_end, repo_end)?;
    if bytes.get(repo_end) != Some(&b'/') {
        return None;
    }
    let eternal_start = repo_end + 1;
    let eternal_end = eternal_start + UUID_TEXT_BYTES;
    let eternal_id = parse_uuid_at(bytes, eternal_start, eternal_end)?;
    Some(ParsedBlockUrl {
        end: eternal_end,
        reference: BlockRef::RepoRelative { repo, eternal_id },
        workspace_id: None,
    })
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
        let after_prefix = index + prefix.len();
        let Some(parsed) = parse_repo_relative_block_url(bytes, after_prefix)
            .or_else(|| parse_direct_block_url(bytes, after_prefix))
        else {
            index += 1;
            continue;
        };
        if bytes
            .get(parsed.end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'/'))
        {
            index += 1;
            continue;
        }
        urls.push(BlockUrl {
            range: index..parsed.end,
            reference: parsed.reference,
            workspace_id: parsed.workspace_id,
        });
        index = parsed.end;
    }
    urls
}
