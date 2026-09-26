use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::builds::{self, Build, Fetch, Object, Slot};

use crate::github::{
    Entry, InlineComment, Person, PullRequest, Repository, State, Tone, branch_deleted, hex_color,
    parse_pull_requests, parse_remote, parse_timeline, timeline_images,
};
use crate::keys::key_bytes;
use crate::markdown::{Block, blocks};
use crate::time::{parse_timestamp, relative};

mod a_build_names_the_app_and_the_launcher_by_their_hashes;
mod a_timeline_lists_the_description_reviews_commits_and_events;
mod an_apk_downloads_only_when_the_one_already_there_differs;
mod an_apk_is_refused_when_it_does_not_match_its_hash;
mod images_in_comments_and_tables_are_gathered_in_order;
mod keys_reach_the_running_program_as_terminal_input;
mod markdown_becomes_paragraphs_lists_code_images_and_tables;
mod pull_requests_are_read_from_the_github_api;
mod the_repository_is_read_from_the_origin_remote;
mod timestamps_read_as_relative_times;

fn repository() -> Repository {
    Repository {
        owner: "blockeditor-org".to_owned(),
        name: "be3".to_owned(),
    }
}

#[derive(Default)]
struct Store {
    objects: HashMap<String, Vec<u8>>,
    fetched: Vec<String>,
}

impl Store {
    fn put(&mut self, slot: Slot, bytes: &[u8]) -> Object {
        let hash: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.objects.insert(slot.object_url(&hash), bytes.to_vec());
        Object {
            hash,
            size: bytes.len() as u64,
        }
    }
}

impl Fetch for Store {
    fn fetch(&mut self, url: &str, into: &mut dyn Write) -> Result<(), String> {
        self.fetched.push(url.to_owned());
        let bytes = self.objects.get(url).ok_or_else(|| format!("{url}: 404"))?;
        into.write_all(bytes).map_err(|error| error.to_string())
    }
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("be-launcher-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

fn fetch_apk(path: &Path, slot: Slot, object: &Object, store: &mut Store) -> Result<(), String> {
    builds::fetch_apk(path, slot, object, store, &mut |_, _| {})
}
