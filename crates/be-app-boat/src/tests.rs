//! Editor tests, answering `fetch` with JSON recorded from boat.dev's API (`tests/fixtures`,
//! with made-up names, ids and addresses). No test reaches boat.

use std::time::Duration;

use be_sdk_beui::be_block_content::Root;
use be_sdk_beui::be_block_content::{EditorView, EditorViewContent};
use be_sdk_beui::network::ResponseHead;
use be_sdk_beui::{
    BlockAgent, EditorHost, Failure, OwnedEditor, TerminalHostBe, TerminalHostDownload,
    TerminalHostSource,
};
use be_sdk_test::{BeuiTest, Fetch, HostCall};
use be_type_boat::{BoatContent, BoatSettings, BoatSettingsContent};
use uuid::Uuid;

use crate::root::{BoatRoot, CACHE, Listed};

mod a_block_without_a_key_shows_how_to_make_one;
mod a_development_build_already_in_the_sandbox_is_not_uploaded_again;
mod a_development_build_uploads_its_own_be;
mod a_download_of_be_must_be_plain_https_with_a_sha256;
mod a_key_boat_no_longer_takes_offers_reconnect;
mod a_key_near_its_expiry_warns_two_weeks_before;
mod a_key_past_its_expiry_asks_nothing_and_offers_reconnect;
mod a_key_without_exec_offers_reconnect_when_connecting;
mod a_long_list_is_read_page_by_page;
mod a_refused_start_says_why_until_dismissed;
mod a_resumed_sandbox_gets_its_terminal_back_with_a_rotated_token;
mod a_running_terminal_is_left_alone_and_a_failing_one_is_put_right;
mod a_terminal_of_a_sandbox_no_longer_listed_can_be_removed;
mod a_terminal_whose_agent_cannot_be_removed_is_kept;
mod a_terminal_whose_build_is_missing_gets_it_again;
mod an_empty_list_says_so;
mod an_expired_key_offers_reconnect;
mod an_unchanged_list_is_not_kept_again_until_it_ages;
mod an_upload_url_that_is_not_https_is_refused;
mod connecting_a_terminal_installs_its_host_in_the_sandbox;
mod connecting_checks_the_key_and_keeps_it_for_its_owner_alone;
mod on_the_web_it_says_it_is_not_available;
mod reconnecting_replaces_the_key;
mod reopening_shows_the_kept_list_before_boat_answers;
mod starting_a_stopped_sandbox_asks_first;
mod stopping_a_running_sandbox_stops_it;
mod the_filters_go_in_the_query;
mod the_list_is_kept_and_shown_while_offline;
mod the_list_shows_each_sandbox_and_how_old_it_is;

const ME: &str = include_str!("tests/fixtures/me.json");
const SANDBOXES: &str = include_str!("tests/fixtures/sandboxes.json");
const EMPTY: &str = include_str!("tests/fixtures/sandboxes_empty.json");
const UNAUTHORIZED: &str = include_str!("tests/fixtures/unauthorized.json");
const EXPIRED: &str = include_str!("tests/fixtures/expired.json");
const RESUMING: &str = include_str!("tests/fixtures/resuming.json");
const FIRST_PAGE: &str = include_str!("tests/fixtures/sandboxes_first_page.json");
const LAST_PAGE: &str = include_str!("tests/fixtures/sandboxes_last_page.json");
const PAYMENT_REQUIRED: &str = include_str!("tests/fixtures/payment_required.json");
const UNAME: &str = include_str!("tests/fixtures/command_uname.json");
const DONE: &str = include_str!("tests/fixtures/command_done.json");
const MISSING: &str = include_str!("tests/fixtures/command_missing.json");
const PRESENT: &str = include_str!("tests/fixtures/command_present.json");
const WRITTEN: &str = include_str!("tests/fixtures/file_written.json");
const UPLOAD_URL: &str = include_str!("tests/fixtures/upload_url.json");
const FORBIDDEN: &str = include_str!("tests/fixtures/forbidden.json");

/// The running sandbox in `SANDBOXES`.
const RUNNING: &str = "bx_k7m2p9qa";
const DEPLOYED_BE: &str = "https://be3.example/data/tree/be-x86_64-unknown-linux-gnu/be";
const BE_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
const PHRASE: &str =
    "abandon ability able about above absent absorb abstract absurd abuse access accident";
const UPLOAD_URL_HTTP: &str = include_str!("tests/fixtures/upload_url_http.json");
const BUILD_INFO: &str = "{\"id\":\"tree\",\"program_id\":\"be-host\"}\n";

const SECRET: &str = "boat_test_0123456789abcdef";
const NEW_SECRET: &str = "boat_test_fedcba9876543210";
const LIST: &str = "https://boat.dev/api/v1/sandboxes?limit=200";

struct Boat {
    test: BeuiTest<BoatRoot>,
    block: Uuid,
    view: Uuid,
    /// The key the editor is expected to call boat with.
    secret: &'static str,
    /// Every command line the editor ran in a sandbox, none of which may hold a secret.
    lines: Vec<String>,
}

impl Drop for Boat {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let secrets = ["first-token", "second-token"]
            .into_iter()
            .chain(PHRASE.split(' '))
            .collect::<Vec<_>>();
        for line in &self.lines {
            let words = line
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
                .collect::<Vec<_>>();
            for secret in &secrets {
                assert!(
                    !words.contains(secret),
                    "a command line holds a secret ({secret}): {line}"
                );
            }
        }
    }
}

fn host(view: Uuid, origin: &str) -> EditorHost {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(view));
    host.set_page_origin(origin.to_owned());
    host
}

impl Boat {
    /// The editor on a new block, on screen, signed out.
    fn new() -> Self {
        Self::opened_at("")
    }

    /// The editor on a new block in a host on the web page at `origin`.
    fn opened_at(origin: &str) -> Self {
        let block = Uuid::new_v4();
        let view = Uuid::new_v4();
        let mut test = BeuiTest::new(OwnedEditor::new(host(view, origin), block));
        test.store().register(be_type_boat::registrations());
        test.store().register(be_type_terminal::registrations());
        test.hold(None, BoatContent::default());
        test.hold(Some(view), EditorView::document(Some(block)));
        test.presence_visible(true);
        test.run();
        Self {
            test,
            block,
            view,
            secret: SECRET,
            lines: Vec::new(),
        }
    }

    /// The same block and view opened again, as by the viewer on another day.
    fn reopened(&self) -> Self {
        let mut test = BeuiTest::new_beside(
            &self.test,
            OwnedEditor::new(host(self.view, ""), self.block),
        );
        test.presence_visible(true);
        test.run();
        Self {
            test,
            block: self.block,
            view: self.view,
            secret: self.secret,
            lines: Vec::new(),
        }
    }

    /// The editor signed in with a key that expires in a year, its first list not answered.
    fn signed_in() -> Self {
        let mut boat = Self::new();
        boat.connect();
        let me = boat.fetch();
        boat.answer(&me, 200, ME);
        boat
    }

    /// Signed in and showing `body` as the list.
    fn listing(body: &str) -> Self {
        let mut boat = Self::signed_in();
        let list = boat.fetch();
        assert_eq!(list.request.url, LIST);
        boat.answer(&list, 200, body);
        boat
    }

    fn connect(&mut self) {
        self.test.click("boat.sign-in.key");
        self.test.type_text(self.secret);
        self.test.run();
        self.test.click("boat.sign-in.connect");
        self.test.run();
    }

    /// The one fetch the editor sent.
    fn fetch(&mut self) -> Fetch {
        self.test
            .settle_until("the editor to ask boat", |test| test.has_fetches());
        let mut fetches = self.test.take_fetches();
        assert_eq!(fetches.len(), 1, "one call at a time: {fetches:?}");
        let fetch = fetches.remove(0);
        assert_eq!(
            fetch.request.header_value("Authorization"),
            Some(format!("Bearer {}", self.secret).as_str())
        );
        fetch
    }

    fn answer(&mut self, fetch: &Fetch, status: u16, body: &str) {
        let head = ResponseHead {
            status,
            headers: Vec::new(),
        };
        let (first, second) = body.as_bytes().split_at(body.len() / 2);
        self.test
            .respond(fetch, head, [first.to_vec(), second.to_vec()]);
        self.test.run();
        self.test.settle();
    }

    fn offline(&mut self, fetch: &Fetch) {
        self.test
            .fail_fetch(fetch, Failure::Failed("the network is down".to_owned()));
        self.test.run();
        self.test.settle();
    }

    /// Lets `by` pass, as the editor's poll sees it.
    fn wait(&mut self, by: Duration) {
        self.test.advance(by);
    }

    fn label(&self, test_id: &str) -> String {
        self.test.label(test_id)
    }

    fn kept(&self) -> Option<Listed> {
        self.test
            .content::<EditorViewContent>(Some(self.view))
            .root()
            .state(CACHE)
            .and_then(|state| state.value())
    }

    fn settings_blocks(&self) -> Vec<Uuid> {
        let store = self.test.store();
        store
            .created()
            .into_iter()
            .filter(|block| {
                store
                    .block(*block)
                    .is_some_and(|info| info.block_type == BoatSettings::CONTENT_TYPE)
            })
            .collect()
    }
}

impl Boat {
    /// The key the viewer's settings keep for the block's own account.
    fn kept_key(&self) -> Option<be_type_boat::BoatKey> {
        let blocks = self.settings_blocks();
        assert!(
            blocks.len() <= 1,
            "one settings block for the kind: {blocks:?}"
        );
        let block = *blocks.first()?;
        self.test
            .content::<BoatSettingsContent>(Some(block))
            .root()
            .key("")
            .cloned()
    }
}

/// A command the editor ran in a sandbox: the fetch and the command line.
struct Command {
    fetch: Fetch,
    sandbox: String,
    line: String,
}

/// A file the editor wrote in a sandbox.
struct Written {
    fetch: Fetch,
    path: String,
    content: String,
}

impl Boat {
    fn command(&mut self) -> Command {
        let fetch = self.fetch();
        assert_eq!(fetch.request.method, "POST");
        let sandbox = fetch
            .request
            .url
            .strip_prefix("https://boat.dev/api/v1/sandboxes/")
            .and_then(|rest| rest.strip_suffix("/commands"))
            .unwrap_or_else(|| panic!("a command, not {}", fetch.request.url))
            .to_owned();
        let body: serde_json::Value = serde_json::from_slice(&fetch.request.body).unwrap();
        let line = body["command"].as_str().unwrap().to_owned();
        self.lines.push(line.clone());
        Command {
            fetch,
            sandbox,
            line,
        }
    }

    fn written(&mut self) -> Written {
        let fetch = self.fetch();
        assert_eq!(fetch.request.method, "PUT");
        assert_eq!(
            fetch.request.url,
            format!("https://boat.dev/api/v1/sandboxes/{RUNNING}/files")
        );
        let body: serde_json::Value = serde_json::from_slice(&fetch.request.body).unwrap();
        Written {
            path: body["path"].as_str().unwrap().to_owned(),
            content: body["content"].as_str().unwrap().to_owned(),
            fetch,
        }
    }

    /// Answers the file written next with `WRITTEN`, checking where it went.
    fn write_answered(&mut self, path: &str) -> String {
        let written = self.written();
        assert_eq!(written.path, path);
        self.answer(&written.fetch, 200, WRITTEN);
        written.content
    }

    /// Answers the command run next with `body`, checking it starts with `start`.
    fn ran(&mut self, start: &str, body: &str) -> String {
        let command = self.command();
        assert_eq!(command.sandbox, RUNNING);
        assert!(
            command.line.starts_with(start),
            "{start:?} was expected, not {:?}",
            command.line
        );
        self.answer(&command.fetch, 200, body);
        command.line
    }

    /// The host call the editor made, picked by `pick`.
    fn host_call<T>(&mut self, what: &str, pick: impl Fn(HostCall) -> Result<T, HostCall>) -> T {
        self.test.settle();
        let mut picked = self.test.take_requests_where(pick);
        assert_eq!(picked.len(), 1, "{what}");
        picked.remove(0)
    }

    /// Presses Connect on the running sandbox and answers until the agent is made, answering
    /// the terminal block it was made for.
    fn connect_terminal(&mut self, agent: Uuid) -> Uuid {
        self.test
            .click(&format!("boat.sandbox.{RUNNING}.terminal.connect"));
        self.test.run();
        let (reply, block, name) = self.host_call("the agent", |call| match call {
            HostCall::CreateBlockAgent { reply, block, name } => Ok((reply, block, name)),
            call => Err(call),
        });
        assert_eq!(name, format!("boat VM {RUNNING}"));
        let block = Uuid::from_bytes(block);
        self.test.answer(
            reply,
            Ok(BlockAgent {
                agent: agent.into_bytes(),
                credentials: credentials("first-token"),
            }),
        );
        self.test.run();
        block
    }

    /// Answers `terminal-host-binary` with `source`.
    fn answer_be(&mut self, source: TerminalHostSource) {
        let (reply, arch) = self.host_call("be", |call| match call {
            HostCall::TerminalHostBinary { reply, arch } => Ok((reply, arch)),
            call => Err(call),
        });
        assert_eq!(arch, "x86_64");
        self.test.answer(
            reply,
            Ok(TerminalHostBe {
                source,
                build_info: BUILD_INFO.to_owned(),
            }),
        );
        self.test.run();
    }

    /// Answers the files and commands that put the units and credentials in place, answering
    /// the credentials written. `units` is a Connect's, with the link to the `tree` build.
    fn install_answered(&mut self, block: Uuid, units: bool) -> serde_json::Value {
        if units {
            let service = self.write_answered("/home/user/.cache/be3/be-terminal@.service");
            assert!(service.contains("ExecStart="));
            assert!(service.contains("/home/user/.local/lib/be3/blocks/%i/be %i terminal-host"));
            assert!(service.contains("EnvironmentFile=-/home/user/.config/be3/gateway"));
            assert!(service.contains("StartLimitBurst="));
            let path = self.write_answered("/home/user/.cache/be3/be-terminal@.path");
            assert!(path.contains("PathExists=/run/be3/terminal-host-%i.json"));
        }
        let credentials =
            self.write_answered(&format!("/home/user/.cache/be3/terminal-host-{block}.json"));
        let parsed: serde_json::Value = serde_json::from_str(&credentials).unwrap();
        let install = self.command();
        assert!(install.line.contains(&format!(
            "sudo install -m 0600 -o user -g user /home/user/.cache/be3/terminal-host-{block}.json \
             /run/be3/terminal-host-{block}.json"
        )));
        assert!(install.line.contains(&format!(
            "trap 'rm -f /home/user/.cache/be3/terminal-host-{block}.json' EXIT"
        )));
        assert!(
            install
                .line
                .contains("sudo install -d -m 0700 -o user -g user /run/be3")
        );
        assert!(
            install
                .line
                .contains(&format!("rm -f /run/be3/terminal-host-{block}.status")),
            "a standing from before is forgotten"
        );
        assert_eq!(
            install.line.contains(&format!(
                "ln -sfn ../builds/tree /home/user/.local/lib/be3/blocks/{block}"
            )),
            units,
            "{}",
            install.line
        );
        assert_eq!(
            install.line.contains("enable --now"),
            units,
            "{}",
            install.line
        );
        assert!(
            install
                .line
                .contains(&format!("systemctl restart be-terminal@{block}.service"))
        );
        self.answer(&install.fetch, 200, DONE);
        parsed
    }

    /// Connects a terminal to the running sandbox with a deployed build's `be`.
    fn connected(&mut self, agent: Uuid) -> Uuid {
        let block = self.connect_terminal(agent);
        self.ran("uname -m", UNAME);
        self.answer_be(TerminalHostSource::Download(TerminalHostDownload {
            url: DEPLOYED_BE.to_owned(),
            sha256: BE_SHA256.to_owned(),
        }));
        self.ran("set -e", DONE);
        self.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
        self.install_answered(block, true);
        block
    }

    /// Answers the check of what the sandbox holds of `block` with `body`, and the host's
    /// build as `tree`.
    fn checked(&mut self, block: Uuid, body: &str) {
        let line = self.ran("rm -f /home/user/.cache/be3/terminal-host-", body);
        assert!(line.contains(&format!("/run/be3/terminal-host-{block}.json")));
        let reply = self.host_call("the host's build", |call| match call {
            HostCall::TerminalHostBuild { reply } => Ok(reply),
            call => Err(call),
        });
        self.test.answer(reply, Ok(BUILD_INFO.to_owned()));
        self.test.run();
        self.test.settle();
    }

    /// The viewer's terminals as their settings keep them.
    fn kept_terminals(&self) -> Vec<(String, be_type_boat::TerminalAgent)> {
        let Some(block) = self.settings_blocks().first().copied() else {
            return Vec::new();
        };
        self.test
            .content::<BoatSettingsContent>(Some(block))
            .root()
            .terminals
            .iter()
            .map(|(block, terminal)| (block.clone(), terminal.clone()))
            .collect()
    }
}

fn credentials(token: &str) -> String {
    serde_json::json!({
        "server": "https://api.be3.example",
        "organization": "0b7e5c1a-3f2d-4c8e-9a61-5d4f2e8b7c31",
        "account": "5d4f2e8b-7c30-4c8e-9a61-0b7e5c1a3f2d",
        "token": token,
        "recovery_phrase": PHRASE,
    })
    .to_string()
}

/// A finished command that printed `stdout`.
fn said(stdout: &str) -> String {
    let mut done: serde_json::Value = serde_json::from_str(DONE).unwrap();
    done["stdout"] = stdout.into();
    done.to_string()
}

impl Boat {
    /// Stops and starts the running sandbox as the list sees it.
    fn resumed(&mut self) {
        let stopped = SANDBOXES.replacen("\"state\": \"idle\"", "\"state\": \"archived\"", 1);
        self.wait(Duration::from_secs(15));
        let list = self.fetch();
        self.answer(&list, 200, &stopped);
        self.wait(Duration::from_secs(15));
        let list = self.fetch();
        self.answer(&list, 200, SANDBOXES);
    }
}
