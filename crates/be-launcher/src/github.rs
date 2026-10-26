use beui::Color32;
use serde_json::Value;

use crate::markdown::{self, Block, blocks};
use crate::net::{Network, get};
use crate::time::parse_timestamp;

const API: &str = "https://api.github.com";
const PAGE_SIZE: usize = 100;
const TIMELINE_PAGES: usize = 5;
const LABEL_FALLBACK: Color32 = Color32::from_rgb(110, 118, 129);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Repository {
    pub(crate) owner: String,
    pub(crate) name: String,
}

impl Repository {
    pub(crate) fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Filter {
    Open,
    Closed,
}

impl Filter {
    fn query(self) -> &'static str {
        match self {
            Filter::Open => "open",
            Filter::Closed => "closed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Open,
    Draft,
    Merged,
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Label {
    pub(crate) name: String,
    pub(crate) color: Color32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Person {
    pub(crate) login: String,
    pub(crate) avatar: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PullRequest {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) state: State,
    pub(crate) author: Person,
    pub(crate) branch: String,
    pub(crate) head_sha: String,
    pub(crate) same_repository: bool,
    pub(crate) labels: Vec<Label>,
    pub(crate) created: i64,
    pub(crate) updated: i64,
    pub(crate) url: String,
    pub(crate) body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Neutral,
    Approved,
    ChangesRequested,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InlineComment {
    pub(crate) author: String,
    pub(crate) location: String,
    pub(crate) body: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Entry {
    Comment {
        author: Person,
        verb: String,
        tone: Tone,
        when: i64,
        url: String,
        body: Vec<Block>,
        inline: Vec<InlineComment>,
    },
    Commit {
        sha: String,
        message: String,
    },
    Event {
        actor: String,
        text: String,
        when: i64,
    },
}

pub(crate) fn timeline_images(entries: &[Entry]) -> Vec<String> {
    let mut found = Vec::new();
    for entry in entries {
        if let Entry::Comment { body, inline, .. } = entry {
            markdown::images(body, &mut found);
            for comment in inline {
                markdown::images(&comment.body, &mut found);
            }
        }
    }
    found
}

pub(crate) const BRANCH_DELETED: &str = "deleted the branch";
pub(crate) const BRANCH_RESTORED: &str = "restored the branch";

pub(crate) fn branch_deleted(entries: &[Entry]) -> bool {
    entries
        .iter()
        .rev()
        .find_map(|entry| match entry {
            Entry::Event { text, .. } if text == BRANCH_DELETED => Some(true),
            Entry::Event { text, .. } if text == BRANCH_RESTORED => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

pub(crate) struct GitHub {
    repository: Repository,
    token: Option<String>,
}

impl GitHub {
    pub(crate) fn connect() -> Result<Self, String> {
        Ok(Self {
            repository: Repository {
                owner: "blockeditor-org".to_owned(),
                name: "be3".to_owned(),
            },
            token: token(),
        })
    }

    pub(crate) fn repository(&self) -> &Repository {
        &self.repository
    }

    pub(crate) fn pull_requests(&self, filter: Filter) -> Result<Vec<PullRequest>, String> {
        let listed = self.api(&format!(
            "repos/{}/pulls?state={}&sort=updated&direction=desc&per_page={PAGE_SIZE}",
            self.repository.full_name(),
            filter.query()
        ))?;
        Ok(parse_pull_requests(&listed, &self.repository))
    }

    pub(crate) fn timeline(&self, pull_request: &PullRequest) -> Result<Vec<Entry>, String> {
        let repository = self.repository.full_name();
        let number = pull_request.number;
        let mut events = Vec::new();
        for page in 1..=TIMELINE_PAGES {
            let listed = self.api(&format!(
                "repos/{repository}/issues/{number}/timeline?per_page={PAGE_SIZE}&page={page}"
            ))?;
            let listed = listed.as_array().cloned().unwrap_or_default();
            let full = listed.len() == PAGE_SIZE;
            events.extend(listed);
            if !full {
                break;
            }
        }
        let review_comments = self.api(&format!(
            "repos/{repository}/pulls/{number}/comments?per_page={PAGE_SIZE}"
        ))?;
        Ok(parse_timeline(
            pull_request,
            &events,
            review_comments.as_array().map_or(&[], Vec::as_slice),
        ))
    }

    pub(crate) fn branch_head(&self, branch: &str) -> Result<String, String> {
        let described = self.api(&format!(
            "repos/{}/branches/{branch}",
            self.repository.full_name()
        ))?;
        described["commit"]["sha"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("GitHub did not say which commit {branch} is on"))
    }

    fn api(&self, path: &str) -> Result<Value, String> {
        let body = download(&format!("{API}/{path}"), self.token.as_deref())
            .map_err(|error| self.explain(error))?;
        serde_json::from_slice(&body).map_err(|error| format!("GitHub sent back {error}"))
    }

    fn explain(&self, error: String) -> String {
        if self.token.is_none() && error.contains("rate limit") {
            format!(
                "{error} Set GITHUB_TOKEN, or sign in with `gh auth login`, to raise the limit."
            )
        } else {
            error
        }
    }
}

pub(crate) fn download(url: &str, token: Option<&str>) -> Result<Vec<u8>, String> {
    let authorization = token.map(|token| format!("Bearer {token}"));
    let mut headers = vec![("Accept", "application/vnd.github+json")];
    if let Some(authorization) = &authorization {
        headers.push(("Authorization", authorization.as_str()));
    }
    let (status, body) = get(&mut Network, url, &headers)?;
    if (200..300).contains(&status) {
        return Ok(body);
    }
    let message = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| body["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_owned());
    Err(format!("{message}."))
}

#[cfg(target_os = "android")]
fn token() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
fn token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|token| !token.trim().is_empty())
        .or_else(|| crate::tasks::capture("gh", &["auth", "token"]).ok())
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

pub(crate) fn parse_pull_requests(listed: &Value, repository: &Repository) -> Vec<PullRequest> {
    listed
        .as_array()
        .map(|listed| {
            listed
                .iter()
                .filter_map(|entry| parse_pull_request(entry, repository))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_pull_request(entry: &Value, repository: &Repository) -> Option<PullRequest> {
    let state = if !entry["merged_at"].is_null() {
        State::Merged
    } else if entry["state"] == "closed" {
        State::Closed
    } else if entry["draft"] == true {
        State::Draft
    } else {
        State::Open
    };
    Some(PullRequest {
        number: entry["number"].as_u64()?,
        title: text(&entry["title"]),
        state,
        author: person(&entry["user"]),
        branch: text(&entry["head"]["ref"]),
        head_sha: text(&entry["head"]["sha"]),
        same_repository: entry["head"]["repo"]["full_name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(&repository.full_name())),
        labels: entry["labels"]
            .as_array()
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| Label {
                        name: text(&label["name"]),
                        color: hex_color(label["color"].as_str().unwrap_or_default()),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        created: time(&entry["created_at"]),
        updated: time(&entry["updated_at"]),
        url: text(&entry["html_url"]),
        body: text(&entry["body"]),
    })
}

pub(crate) fn parse_timeline(
    pull_request: &PullRequest,
    events: &[Value],
    review_comments: &[Value],
) -> Vec<Entry> {
    let description = if pull_request.body.trim().is_empty() {
        vec![Block::Paragraph("No description provided.".to_owned())]
    } else {
        blocks(&pull_request.body)
    };
    let mut entries = vec![Entry::Comment {
        author: pull_request.author.clone(),
        verb: "opened this pull request".to_owned(),
        tone: Tone::Neutral,
        when: pull_request.created,
        url: pull_request.url.clone(),
        body: description,
        inline: Vec::new(),
    }];
    entries.extend(
        events
            .iter()
            .filter_map(|event| parse_event(event, review_comments)),
    );
    entries
}

fn parse_event(event: &Value, review_comments: &[Value]) -> Option<Entry> {
    let actor = event["actor"]["login"]
        .as_str()
        .or_else(|| event["user"]["login"].as_str())
        .unwrap_or("someone")
        .to_owned();
    let when = time(&event["created_at"]);
    let happened = |text: String| {
        Some(Entry::Event {
            actor: actor.clone(),
            text,
            when,
        })
    };
    match event["event"].as_str()? {
        "commented" => Some(Entry::Comment {
            author: person(&event["user"]),
            verb: "commented".to_owned(),
            tone: Tone::Neutral,
            when,
            url: text(&event["html_url"]),
            body: blocks(&text(&event["body"])),
            inline: Vec::new(),
        }),
        "reviewed" => {
            let (verb, tone) = match event["state"].as_str().unwrap_or_default() {
                "approved" | "APPROVED" => ("approved these changes", Tone::Approved),
                "changes_requested" | "CHANGES_REQUESTED" => {
                    ("requested changes", Tone::ChangesRequested)
                }
                "dismissed" | "DISMISSED" => ("left a dismissed review", Tone::Neutral),
                _ => ("reviewed", Tone::Neutral),
            };
            let id = event["id"].as_u64();
            let inline: Vec<InlineComment> = review_comments
                .iter()
                .filter(|comment| id.is_some() && comment["pull_request_review_id"].as_u64() == id)
                .map(inline_comment)
                .collect();
            let body = blocks(&text(&event["body"]));
            if body.is_empty() && inline.is_empty() && tone == Tone::Neutral {
                return None;
            }
            Some(Entry::Comment {
                author: person(&event["user"]),
                verb: verb.to_owned(),
                tone,
                when: time(&event["submitted_at"]),
                url: text(&event["html_url"]),
                body,
                inline,
            })
        }
        "committed" => Some(Entry::Commit {
            sha: text(&event["sha"]).chars().take(7).collect(),
            message: text(&event["message"])
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned(),
        }),
        "labeled" => happened(format!("added the {} label", text(&event["label"]["name"]))),
        "unlabeled" => happened(format!(
            "removed the {} label",
            text(&event["label"]["name"])
        )),
        "merged" => happened("merged this pull request".to_owned()),
        "closed" => happened("closed this pull request".to_owned()),
        "reopened" => happened("reopened this pull request".to_owned()),
        "head_ref_force_pushed" => happened("force-pushed the branch".to_owned()),
        "head_ref_deleted" => happened(BRANCH_DELETED.to_owned()),
        "head_ref_restored" => happened(BRANCH_RESTORED.to_owned()),
        "ready_for_review" => happened("marked this pull request as ready for review".to_owned()),
        "convert_to_draft" => happened("marked this pull request as a draft".to_owned()),
        "review_requested" => {
            let reviewer = event["requested_reviewer"]["login"]
                .as_str()
                .or_else(|| event["requested_team"]["name"].as_str())
                .unwrap_or("someone");
            happened(format!("requested a review from {reviewer}"))
        }
        "assigned" => happened(format!("assigned {}", text(&event["assignee"]["login"]))),
        "renamed" => happened(format!(
            "changed the title from \"{}\" to \"{}\"",
            text(&event["rename"]["from"]),
            text(&event["rename"]["to"])
        )),
        "cross-referenced" => {
            let source = &event["source"]["issue"];
            happened(format!(
                "mentioned this in #{} {}",
                source["number"].as_u64().unwrap_or_default(),
                text(&source["title"])
            ))
        }
        _ => None,
    }
}

fn inline_comment(comment: &Value) -> InlineComment {
    let path = text(&comment["path"]);
    let location = match comment["line"]
        .as_u64()
        .or_else(|| comment["original_line"].as_u64())
    {
        Some(line) => format!("{path}:{line}"),
        None => path,
    };
    InlineComment {
        author: text(&comment["user"]["login"]),
        location,
        body: blocks(&text(&comment["body"])),
    }
}

fn person(user: &Value) -> Person {
    Person {
        login: user["login"].as_str().unwrap_or("ghost").to_owned(),
        avatar: text(&user["avatar_url"]),
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn time(value: &Value) -> i64 {
    value.as_str().and_then(parse_timestamp).unwrap_or_default()
}

pub(crate) fn hex_color(hex: &str) -> Color32 {
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range)
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
    };
    match (channel(0..2), channel(2..4), channel(4..6)) {
        (Some(red), Some(green), Some(blue)) if hex.len() == 6 => {
            Color32::from_rgb(red, green, blue)
        }
        _ => LABEL_FALLBACK,
    }
}
