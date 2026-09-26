use serde_json::json;

use crate::github::{
    Entry, Person, PullRequest, Repository, State, Tone, branch_deleted, hex_color,
    parse_pull_requests, parse_remote, parse_timeline,
};
use crate::markdown::{Block, blocks};
use crate::pane::with_carriage_returns;
use crate::time::{parse_timestamp, relative};

mod a_timeline_lists_the_description_reviews_commits_and_events;
mod markdown_becomes_paragraphs_lists_code_images_and_tables;
mod piped_output_returns_to_the_first_column_on_each_newline;
mod pull_requests_are_read_from_the_github_api;
mod the_repository_is_read_from_the_origin_remote;
mod timestamps_read_as_relative_times;

fn repository() -> Repository {
    Repository {
        owner: "blockeditor-org".to_owned(),
        name: "be3".to_owned(),
    }
}
