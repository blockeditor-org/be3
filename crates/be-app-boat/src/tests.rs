//! Editor tests, answering `fetch` with JSON recorded from boat.dev's API (`tests/fixtures`,
//! with made-up names, ids and addresses). No test reaches boat.

use std::time::Duration;

use be_sdk_beui::be_block::{EditorView, EditorViewContent};
use be_sdk_beui::network::ResponseHead;
use be_sdk_beui::{EditorHost, Failure, OwnedEditor, PageOriginWatch};
use be_sdk_test::{BeuiTest, Fetch};
use be_sdk_beui::be_block::Root;
use be_type_boat::{BoatContent, BoatSettings, BoatSettingsContent};
use uuid::Uuid;

use crate::root::{BoatRoot, CACHE, Listed};

mod a_block_without_a_key_shows_how_to_make_one;
mod a_key_near_its_expiry_warns_two_weeks_before;
mod a_key_past_its_expiry_asks_nothing_and_offers_reconnect;
mod an_empty_list_says_so;
mod an_expired_key_offers_reconnect;
mod connecting_checks_the_key_and_keeps_it_for_its_owner_alone;
mod on_the_web_it_says_it_is_not_available;
mod starting_a_stopped_sandbox_asks_first;
mod stopping_a_running_sandbox_stops_it;
mod the_list_is_kept_and_shown_while_offline;
mod the_list_shows_each_sandbox_and_how_old_it_is;

const ME: &str = include_str!("tests/fixtures/me.json");
const SANDBOXES: &str = include_str!("tests/fixtures/sandboxes.json");
const EMPTY: &str = include_str!("tests/fixtures/sandboxes_empty.json");
const UNAUTHORIZED: &str = include_str!("tests/fixtures/unauthorized.json");
const EXPIRED: &str = include_str!("tests/fixtures/expired.json");
const RESUMING: &str = include_str!("tests/fixtures/resuming.json");

const SECRET: &str = "boat_test_0123456789abcdef";
const LIST: &str = "https://boat.dev/api/v1/sandboxes?limit=200";

struct Boat {
    test: BeuiTest<BoatRoot>,
    view: Uuid,
}

impl Boat {
    /// The editor on a new block, on screen, signed out.
    fn new() -> Self {
        let block = Uuid::new_v4();
        let view = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_view_block(Some(view));
        let mut test = BeuiTest::new(OwnedEditor::new(host, block));
        test.store().register(be_type_boat::registrations());
        test.hold(None, BoatContent::default());
        test.hold(Some(view), EditorView::document(Some(block)));
        test.presence_visible(true);
        test.run();
        Self { test, view }
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
        self.test.type_text(SECRET);
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
            Some(format!("Bearer {SECRET}").as_str())
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
                store.block(*block).is_some_and(|info| {
                    info.block_type == BoatSettings::CONTENT_TYPE
                })
            })
            .collect()
    }
}
