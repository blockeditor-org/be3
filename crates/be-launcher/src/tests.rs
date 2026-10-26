use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::builds::{self, Installed, Slot};
use crate::github::{
    Entry, InlineComment, Person, PullRequest, Repository, State, Tone, branch_deleted, hex_color,
    parse_pull_requests, parse_timeline, timeline_images,
};
use crate::helper::{self, Helper, Manifest, Product};
use crate::markdown::{Block, blocks};
use crate::net::Http;
use crate::time::{parse_timestamp, relative};

mod a_file_is_refused_when_it_does_not_match_its_hash;
mod a_manifest_names_its_files_by_hash_inside_the_app;
mod a_timeline_lists_the_description_reviews_commits_and_events;
mod images_in_comments_and_tables_are_gathered_in_order;
mod markdown_becomes_paragraphs_lists_code_images_and_tables;
mod only_the_files_not_already_here_are_downloaded;
mod pull_requests_are_read_from_the_github_api;
mod pull_requests_open_in_place_of_the_one_shown_unless_ctrl_is_held;
mod the_build_helper_is_asked_with_the_gateway_key;
mod the_image_viewer_keeps_its_close_button_on_a_narrow_screen;
mod timestamps_read_as_relative_times;

fn repository() -> Repository {
    Repository {
        owner: "blockeditor-org".to_owned(),
        name: "be3".to_owned(),
    }
}

struct Asked {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
}

#[derive(Default)]
struct Store {
    answers: HashMap<String, (u16, Vec<u8>)>,
    asked: Vec<Asked>,
}

impl Store {
    fn put(&mut self, bytes: &[u8]) -> helper::File {
        let sha256: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.answers
            .insert(helper::file_url(&sha256), (200, bytes.to_vec()));
        helper::File {
            path: String::new(),
            sha256,
            size: bytes.len() as u64,
            executable: false,
        }
    }

    fn answer(&mut self, url: &str, status: u16, body: &str) {
        self.answers
            .insert(url.to_owned(), (status, body.as_bytes().to_vec()));
    }

    fn urls(&self) -> Vec<&str> {
        self.asked.iter().map(|asked| asked.url.as_str()).collect()
    }
}

impl Http for Store {
    fn send(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        into: &mut dyn Write,
    ) -> Result<u16, String> {
        self.asked.push(Asked {
            method: method.to_owned(),
            url: url.to_owned(),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        });
        let (status, body) = self.answers.get(url).cloned().unwrap_or((404, Vec::new()));
        into.write_all(&body).map_err(|error| error.to_string())?;
        Ok(status)
    }
}

fn manifest(files: Vec<(&str, helper::File)>) -> Manifest {
    Manifest {
        product: "android".to_owned(),
        commit: "c".repeat(40),
        files: files
            .into_iter()
            .map(|(path, file)| helper::File {
                path: path.to_owned(),
                ..file
            })
            .collect(),
    }
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("be-launcher-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}
