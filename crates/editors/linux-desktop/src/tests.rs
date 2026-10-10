use block_editor_beui::be_block::{
    EditorView, EditorViewContent, LINUX_DESKTOP_EDITOR, Root, Settings, SettingsContent,
    WORKSPACE_EDITOR,
};
use block_editor_beui::beui::{Document, Key, KeyChord, Modifiers, NodeId, Rect};
use block_editor_beui::{
    AudioOutput, BlockInfo, BlockParent, ChildContent, Editor, EditorHost, HostImage, HostProgram,
    HostWindow, HostWindowId, HostWindows, IncomingNotification, Media, MediaLevel, MediaLevels,
    MediaRequest, NotificationInbox, NotificationReport, NotificationRequest, NotificationSignal,
    Notifications, PlayerCommand, Power, PowerAction, PowerAvailability, ProgramAction, Programs,
    WindowAction,
};
use block_plugin_api::Size;
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::LinuxDesktopApp;
use crate::app::media::{BINDINGS, BRIGHTNESS_STEP, VOLUME_STEP};

mod a_block_shown_on_the_desktop_opens_in_its_own_window;
mod a_calendar_the_desktop_no_longer_holds_is_replaced;
mod a_first_desktop_with_no_sessions_offers_a_new_one;
mod a_held_volume_key_keeps_turning_the_volume;
mod a_locked_screen_shows_no_notification_toasts;
mod a_notification_toast_shows_above_the_bar_until_its_time_is_up;
mod a_query_no_program_matches_runs_as_a_command;
mod a_session_chosen_from_the_menu_opens_in_a_window_and_closing_it_keeps_the_session;
mod a_super_tap_toggles_the_launcher_even_from_a_program_with_the_keyboard;
mod a_toast_answers_its_actions_and_hides_while_the_history_is_open;
mod a_volume_key_shows_the_level_the_host_reports_until_it_fades;
mod a_window_the_host_focuses_leads_the_window_switcher;
mod alt_tab_switches_to_the_window_two_back_once_alt_is_let_go;
mod clicking_the_clock_opens_the_desktops_calendar_and_clicking_away_closes_it;
mod escape_leaves_the_window_switcher_without_switching;
mod super_f_toggles_fullscreen_on_the_window_with_the_keyboard;
mod the_calendar_popup_fits_a_narrow_screen;
mod the_desktop_starts_with_nothing_open_but_its_bar;
mod the_media_keys_ask_the_host_even_from_a_program_with_the_keyboard;
mod the_notifications_button_lists_what_arrived_and_answers_it;
mod the_power_menu_asks_before_ending_the_session;
mod the_power_menu_locks_the_screen_without_asking;
mod the_power_menu_offers_only_what_the_host_allows;
mod the_programs_button_opens_the_launcher_on_the_programs_the_host_lists;
mod the_volume_in_the_bar_follows_the_host_and_opens_its_controls;
mod the_volume_popup_asks_the_host_for_a_level_a_mute_and_an_output;
mod the_volume_shows_on_every_monitor;

const MAX_TAB: u64 = 64;
const WINDOW_TABS: u64 = 1 << 41;
const EVERYTHING: PowerAvailability = PowerAvailability {
    lock: true,
    suspend: true,
    restart: true,
    power_off: true,
    log_out: true,
};
const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);
const DEFAULT_ACTION: &str = "default";
const NOON: u64 = 1_768_478_400;
const SPEAKERS: &str = "alsa_output.pci-0000_00_1f.3.analog-stereo";
const HEADPHONES: &str = "bluez_output.headphones";

struct Fixture {
    test: BeuiTest<LinuxDesktopApp>,
    host: EditorHost,
    client: Uuid,
    settings: Uuid,
    notified: u64,
}

impl Fixture {
    fn new() -> Self {
        let desktop = Uuid::new_v4();
        let client = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(client);
        host.set_view_block(Some(desktop));
        let editor = Editor::new(host.clone(), desktop);
        let mut test = BeuiTest::new(editor);
        test.hold(None, EditorView::document(LINUX_DESKTOP_EDITOR, None));
        Self {
            test,
            host,
            client,
            settings: Uuid::new_v4(),
            notified: 0,
        }
    }

    fn with_sessions(mut self, sessions: &[(&str, Uuid)]) -> (Self, Vec<Uuid>) {
        let store = self.test.store();
        store.add_block(BlockInfo::new(
            self.settings,
            Settings::CONTENT_TYPE,
            BlockParent::Root,
        ));
        let mut settings = SettingsContent::default();
        let mut profiles = Vec::new();
        for (name, editor) in sessions {
            let profile = Uuid::new_v4();
            store.add_block(BlockInfo {
                name: Some((*name).to_owned()),
                ..BlockInfo::new(
                    profile,
                    EditorView::CONTENT_TYPE,
                    BlockParent::Block(self.settings),
                )
            });
            store.hold(Some(profile), EditorView::document(*editor, None));
            settings.apply(&Settings::add_profile(*editor, self.client, profile));
            profiles.push(profile);
        }
        self.test.hold(Some(self.settings), settings);
        (self, profiles)
    }

    fn with_windows(ids: &[u64]) -> Self {
        let mut fixture = Self::new();
        fixture.settle();
        for focused in std::iter::once(None).chain(ids.iter().copied().map(Some)) {
            let windows = ids
                .iter()
                .map(|id| host_window(*id, focused == Some(*id)))
                .collect();
            fixture.test.set_host_value::<HostWindows>(&windows);
            fixture.settle();
        }
        fixture.test.take_sent();
        fixture
    }

    fn focused_windows(&self) -> Vec<HostWindowId> {
        self.test
            .actions::<WindowAction>()
            .into_iter()
            .filter_map(|action| match action {
                WindowAction::Focus(window) => Some(window),
                _ => None,
            })
            .collect()
    }

    fn allow_power(&mut self, power: PowerAvailability) {
        self.test.set_host_value::<Power>(&power);
        self.settle();
    }

    fn settle(&mut self) {
        self.test.run();
    }

    fn hear(&mut self, level: f32, muted: bool, default_output: &str) {
        self.test.set_host_value::<Media>(&MediaLevels {
            output: Some(MediaLevel { level, muted }),
            outputs: vec![
                AudioOutput {
                    id: SPEAKERS.to_owned(),
                    name: "Speakers".to_owned(),
                },
                AudioOutput {
                    id: HEADPHONES.to_owned(),
                    name: "Headphones".to_owned(),
                },
            ],
            default_output: Some(default_output.to_owned()),
            ..MediaLevels::default()
        });
        self.settle();
    }

    fn notify(&mut self, arrived: &[IncomingNotification]) {
        let requests = arrived
            .iter()
            .map(|incoming| {
                self.notified += 1;
                (
                    self.notified,
                    NotificationRequest::Notify(Box::new(incoming.clone())),
                )
            })
            .collect();
        self.test
            .set_host_value::<Notifications>(&NotificationInbox { requests });
        self.settle();
    }

    fn signals(&mut self) -> Vec<NotificationSignal> {
        self.test
            .take_actions::<NotificationReport>()
            .into_iter()
            .flat_map(|report| report.signals)
            .collect()
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| text_within(document, root, words).is_some())
    }

    fn click_text(&mut self, words: &str) {
        let document = self.test.document();
        let node = document
            .root()
            .and_then(|root| text_within(document, root, words))
            .unwrap_or_else(|| panic!("{words:?} is on screen"));
        let center = document
            .node_rect(node)
            .expect("the text is laid out")
            .center();
        self.test.click_at(center);
        self.settle();
    }

    fn placed_blocks(&self) -> Vec<(Uuid, Option<Uuid>, Rect)> {
        self.test
            .children()
            .iter()
            .filter_map(|placement| match placement.content {
                ChildContent::Block {
                    block_id,
                    view_block,
                    ..
                } => Some((
                    Uuid::from_bytes(block_id),
                    view_block.map(Uuid::from_bytes),
                    Rect::from_min_size(
                        block_editor_beui::pos2(placement.rect.x, placement.rect.y),
                        block_editor_beui::vec2(placement.rect.width, placement.rect.height),
                    ),
                )),
                _ => None,
            })
            .collect()
    }

    fn calendar(&self) -> Option<Uuid> {
        match self.placed_blocks()[..] {
            [] => None,
            [(block, _, _)] => Some(block),
            ref placed => panic!("only the calendar is placed: {placed:?}"),
        }
    }

    fn close_a_tab(&mut self) {
        let document = self.test.document();
        let cross = (0..MAX_TAB)
            .filter_map(|tab| document.find_test_id(&format!("dock.tab.{tab}.close")))
            .find_map(|close| document.node_rect(close))
            .expect("an open tab can be closed");
        self.test.click_at(cross.center());
        self.settle();
    }
}

fn notification(id: u32, app_name: &str, summary: &str, body: &str) -> IncomingNotification {
    IncomingNotification {
        id,
        app_name: app_name.to_owned(),
        summary: summary.to_owned(),
        body: body.to_owned(),
        expire_timeout: -1,
        received: NOON + u64::from(id) * 60,
        ..IncomingNotification::default()
    }
}

fn host_window(id: u64, focused: bool) -> HostWindow {
    HostWindow {
        id: HostWindowId(id),
        title: format!("Program {id}"),
        app_id: "test".to_owned(),
        parent: None,
        size: Size {
            width: 320.0,
            height: 200.0,
        },
        fullscreen: None,
        responding: true,
        focused,
    }
}

fn programs() -> Vec<HostProgram> {
    const SIDE: u32 = 32;
    let icon = (0..SIDE * SIDE).flat_map(|at| match (at % SIDE + at / SIDE) % 8 < 4 {
        true => [0x2e, 0x7d, 0x32, 0xff],
        false => [0x1b, 0x5e, 0x20, 0xff],
    });
    vec![
        HostProgram {
            id: "files.desktop".to_owned(),
            name: "Files".to_owned(),
            generic_name: "File Manager".to_owned(),
            comment: String::new(),
            keywords: vec!["folder".to_owned()],
            icon: None,
        },
        HostProgram {
            id: "foot.desktop".to_owned(),
            name: "Foot".to_owned(),
            generic_name: "Terminal".to_owned(),
            comment: "A fast terminal emulator".to_owned(),
            keywords: vec!["shell".to_owned()],
            icon: Some(HostImage {
                width: SIDE,
                height: SIDE,
                rgba: icon.collect(),
            }),
        },
    ]
}

fn window_tab(id: u64) -> u64 {
    WINDOW_TABS + id
}

fn text_within(document: &Document, id: NodeId, words: &str) -> Option<NodeId> {
    if document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
    {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_within(document, child, words))
}

fn text_under(document: &Document, id: NodeId) -> Option<String> {
    if let Some(node) = document.arena.kind_of(id) {
        return Some(document.text(node).to_owned());
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_under(document, child))
}

fn toast(id: u32) -> String {
    format!("toast.{}", (1_u64 << 32) + u64::from(id))
}
