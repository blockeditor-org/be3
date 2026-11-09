//! Editor tests, answering `fetch` with JSON recorded from boat.dev's API (`tests/fixtures`,
//! with made-up names, ids and addresses). No test reaches boat.

use std::time::Duration;

use be_sdk_beui::be_block::Root;
use be_sdk_beui::be_block::{EditorView, EditorViewContent};
use be_sdk_beui::network::ResponseHead;
use be_sdk_beui::{EditorHost, Failure, OwnedEditor};
use be_sdk_test::{BeuiTest, Fetch};
use be_type_boat::{BoatContent, BoatSettings, BoatSettingsContent};
use uuid::Uuid;

use crate::root::{BoatRoot, CACHE, Listed};

mod a_block_without_a_key_shows_how_to_make_one;
mod a_key_boat_no_longer_takes_offers_reconnect;
mod a_key_near_its_expiry_warns_two_weeks_before;
mod a_key_past_its_expiry_asks_nothing_and_offers_reconnect;
mod a_long_list_is_read_page_by_page;
mod a_refused_start_says_why_until_dismissed;
mod an_empty_list_says_so;
mod an_expired_key_offers_reconnect;
mod an_unchanged_list_is_not_kept_again_until_it_ages;
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

const SECRET: &str = "boat_test_0123456789abcdef";
const NEW_SECRET: &str = "boat_test_fedcba9876543210";
const LIST: &str = "https://boat.dev/api/v1/sandboxes?limit=200";

struct Boat {
    test: BeuiTest<BoatRoot>,
    block: Uuid,
    view: Uuid,
    /// The key the editor is expected to call boat with.
    secret: &'static str,
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
        test.hold(None, BoatContent::default());
        test.hold(Some(view), EditorView::document(Some(block)));
        test.presence_visible(true);
        test.run();
        Self {
            test,
            block,
            view,
            secret: SECRET,
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
        assert!(blocks.len() <= 1, "one settings block for the kind: {blocks:?}");
        let block = *blocks.first()?;
        self.test
            .content::<BoatSettingsContent>(Some(block))
            .root()
            .key("")
            .cloned()
    }
}
