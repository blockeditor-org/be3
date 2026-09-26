use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::builds::{self, Build, Downloaded, Entry as BuildEntry, Fetch, Object, Slot};

use crate::github::{
    Entry, InlineComment, Person, PullRequest, Repository, State, Tone, branch_deleted, hex_color,
    parse_pull_requests, parse_remote, parse_timeline, timeline_images,
};
use crate::keys::key_bytes;
use crate::markdown::{Block, blocks};
use crate::time::{parse_timestamp, relative};

mod a_build_downloads_only_what_changed_and_drops_what_it_no_longer_has;
mod a_build_is_refused_when_a_file_does_not_match_its_hash;
mod a_build_naming_a_file_outside_its_directory_is_refused;
mod a_timeline_lists_the_description_reviews_commits_and_events;
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
    fn put(&mut self, slot: Slot, bytes: &[u8]) -> String {
        let hash: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        self.objects
            .insert(slot.object_url(&hash), encoder.finish().unwrap());
        hash
    }

    fn build(&mut self, slot: Slot, commit: &str, files: &[(&str, &[u8])]) -> Build {
        let launcher = self.put(slot, b"launcher");
        Build {
            commit: commit.to_owned(),
            shell: "shell".to_owned(),
            launcher: Object {
                hash: launcher,
                size: 8,
            },
            files: files
                .iter()
                .map(|(path, bytes)| BuildEntry {
                    path: (*path).to_owned(),
                    hash: self.put(slot, bytes),
                    size: bytes.len() as u64,
                })
                .collect(),
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

fn sync(directory: &Path, slot: Slot, build: &Build, store: &mut Store) -> Result<(), String> {
    builds::sync(directory, slot, build, store, &mut |_, _| {})
}
