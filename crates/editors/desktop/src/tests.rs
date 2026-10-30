use block_editor_beui::be_block::BlockMetadata;
use block_editor_beui::be_block::display_settings::{DisplayMode, Rotation};
use block_editor_beui::be_block::{
    DESKTOP_EDITOR, DisplaySettings, DisplaySettingsContent, Root, Settings, SettingsContent,
    SettingsProfile, SettingsProfileContent, VIEW_EDITOR, WORKSPACE_EDITOR,
};
use block_editor_beui::beui::native::Document;
use block_editor_beui::beui::{Key, KeyChord, Modifiers, NodeId, Rect, Vec2};
use block_editor_beui::{
    AudioOutput, BlockInfo, BlockParent, ChildContent, DisplaysWatch, EditorHost, EpaperWatch,
    HostDisplay, HostImage, HostProgram, HostWindow, HostWindowId, IncomingNotification,
    MediaLevel, MediaLevels, MediaRequest, MediaWatch, NotificationInbox, NotificationRequest,
    NotificationSignal, NotificationsWatch, OwnedEditor, PlayerCommand, PowerAction,
    PowerAvailability, PowerWatch, ProgramAction, ProgramsWatch, WifiLink, WifiNetwork,
    WifiRequest, WifiSecurity, WifiStatus, WifiWatch, WindowAction, WindowKind, WindowOrigin,
    WindowsWatch,
};

use block_editor_beui::be_block::{EditorView, EditorViewContent};
use block_shell::{KeptWindow, saved_windows};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::DesktopApp;
use crate::app::media::{BINDINGS, BRIGHTNESS_STEP, VOLUME_STEP};

mod a_block_shown_on_the_desktop_opens_in_its_own_window;
mod a_calendar_the_desktop_no_longer_holds_is_replaced;
mod a_desktop_resumed_after_a_restart_shows_each_window_where_it_was;
mod a_desktop_started_again_adopts_live_windows_instead_of_placeholders;
mod a_desktop_started_again_adopts_the_windows_the_host_kept;
mod a_first_desktop_with_no_sessions_offers_a_new_one;
mod a_fullscreen_windows_tab_on_another_screen_offers_to_leave_full_screen;
mod a_held_volume_key_keeps_turning_the_volume;
mod a_locked_screen_shows_no_notification_toasts;
mod a_new_window_fills_the_oldest_placeholder_of_its_program;
mod a_notification_that_arrives_on_an_idle_desktop_keeps_its_full_time;
mod a_notification_toast_shows_above_the_bar_until_its_time_is_up;
mod a_program_window_dragged_to_another_display_shows_on_both_on_the_way;
mod a_query_no_program_matches_runs_as_a_command;
mod a_session_chosen_from_the_menu_opens_in_a_window_and_closing_it_keeps_the_session;
mod a_super_tap_toggles_the_launcher_even_from_a_program_with_the_keyboard;
mod a_toast_answers_its_actions_and_hides_while_the_history_is_open;
mod a_volume_key_shows_the_level_the_host_reports_until_it_fades;
mod a_window_launched_from_a_placeholder_takes_its_place;
mod a_window_the_host_focuses_leads_the_window_switcher;
mod a_windows_note_shows_in_its_tab_in_place_of_its_title;
mod a_windows_tab_dragged_onto_a_window_on_another_display_joins_it;
mod a_windows_tab_dragged_onto_another_windows_bar_joins_it;
mod a_windows_tab_menu_makes_it_full_screen;
mod alt_tab_or_a_click_brings_a_fullscreen_window_back_in_front;
mod alt_tab_switches_to_the_window_two_back_once_alt_is_let_go;
mod clicking_the_clock_opens_the_desktops_calendar_and_clicking_away_closes_it;
mod each_display_shows_its_own_bar_at_its_own_foot;
mod escape_closes_the_launcher_and_gives_the_focus_back_to_its_button;
mod escape_leaves_the_window_switcher_without_switching;
mod picking_a_turned_monitor_writes_its_rotation_for_every_device;
mod picking_a_turned_screen_writes_its_rotation_into_this_devices_display_settings;
mod quick_settings_offers_the_keyboard_rotation_and_refresh_the_screen_has;
mod super_f_toggles_fullscreen_on_the_window_with_the_keyboard;
mod the_calendar_popup_fits_a_narrow_screen;
mod the_desktop_away_from_linux_hides_what_its_host_does_not_offer;
mod the_desktop_keeps_each_programs_window_to_bring_back;
mod the_desktop_starts_with_nothing_open_but_its_bar;
mod the_level_stays_up_while_keys_keep_coming_and_fades_rather_than_vanishing;
mod the_media_keys_ask_the_host_even_from_a_program_with_the_keyboard;
mod the_clock_lists_the_notifications_that_arrived_and_answers_them;
mod the_power_row_asks_before_ending_the_session;
mod the_power_row_locks_the_screen_without_asking;
mod the_power_row_offers_only_what_the_host_allows;
mod the_programs_button_opens_the_launcher_on_the_programs_the_host_lists;
mod the_refresh_button_shows_on_an_epaper_display_and_asks_the_host_to_refresh;
mod the_rotate_button_shows_when_the_display_settings_ask_for_it_and_a_screen_is_reported;
mod the_tray_shows_its_items_and_answers_their_clicks_and_menus;
mod the_status_button_shows_the_volume_and_opens_its_controls;
mod the_volume_controls_ask_the_host_for_a_level_a_mute_and_an_output;
mod the_volume_shows_on_every_monitor;
mod the_wifi_tile_lists_the_networks_the_host_reports;
mod the_wifi_list_asks_the_host_to_connect;
mod the_window_picker_lists_displays_then_windows_and_shows_or_hides_them;
mod windows_that_close_as_the_session_ends_are_kept;
mod with_instant_motion_the_level_keeps_its_time_and_goes_without_fading;

const MAX_TAB: u64 = 64;
const WINDOW_TABS: u64 = 1 << 41;
const EVERYTHING: PowerAvailability = PowerAvailability {
    lock: true,
    suspend: true,
    restart: true,
    power_off: true,
    log_out: true,
    ending: false,
};
const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);
const DEFAULT_ACTION: &str = "default";
const NOON: u64 = 1_768_478_400;
const SPEAKERS: &str = "alsa_output.pci-0000_00_1f.3.analog-stereo";
const HEADPHONES: &str = "bluez_output.headphones";

struct Fixture {
    test: BeuiTest<DesktopApp>,
    host: EditorHost,
    client: Uuid,
    settings: Uuid,
    notified: u64,
}

impl Fixture {
    fn new() -> Self {
        Self::holding(EditorView::document(None))
    }

    fn away_from_linux() -> Self {
        Self::unserved(EditorView::document(None))
    }

    fn resumed(saved: EditorViewContent) -> Self {
        let mut fixture = Self::holding(saved);
        fixture.test.set_watched::<ProgramsWatch>(&programs());
        fixture.settle();
        fixture
    }

    fn saved(&self) -> EditorViewContent {
        self.test.content::<EditorViewContent>(None)
    }

    fn kept(&self) -> Vec<KeptWindow> {
        self.saved()
            .root()
            .state("layout")
            .map(saved_windows)
            .unwrap_or_default()
    }

    fn show_windows(&mut self, windows: &[HostWindow]) {
        self.test.set_watched::<WindowsWatch>(&windows.to_vec());
        self.settle();
    }

    fn window_rect(&self, id: u64) -> Option<Rect> {
        self.test
            .children()
            .iter()
            .find(|placement| placement.content == ChildContent::Window(HostWindowId(id)))
            .map(|placement| placement.rect)
    }

    fn placeholder_rect(&self, key: u64) -> Option<Rect> {
        let id = format!("desktop.placeholder.{key}");
        self.test.shown(&id).then(|| self.test.rect_of(&id))
    }

    fn holding(view: EditorViewContent) -> Self {
        let mut fixture = Self::unserved(view);
        fixture.test.set_watched::<ProgramsWatch>(&Vec::new());
        fixture
            .test
            .set_watched::<NotificationsWatch>(&NotificationInbox::default());
        fixture
            .test
            .set_watched::<MediaWatch>(&MediaLevels::default());
        fixture
            .test
            .set_watched::<PowerWatch>(&PowerAvailability::default());
        fixture
    }

    fn unserved(view: EditorViewContent) -> Self {
        let desktop = Uuid::new_v4();
        let client = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(client);
        host.set_view_block(Some(desktop));
        let editor = OwnedEditor::new(host.clone(), desktop);
        let mut test = BeuiTest::new(editor);
        test.hold(None, view);
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
        let mut blocks = Vec::new();
        for (name, editor) in sessions {
            let session = Uuid::new_v4();
            let mut info = BlockInfo::new(
                session,
                EditorView::CONTENT_TYPE,
                BlockParent::Block(self.settings),
            );
            info.metadata = BlockMetadata::view(*editor, Some((*name).to_owned()));
            store.add_block(info);
            store.hold(Some(session), EditorView::document(None));
            settings.apply(&Settings::add_session(*editor, self.client, session));
            blocks.push(session);
        }
        self.test.hold(Some(self.settings), settings);
        (self, blocks)
    }

    fn with_display_settings(
        &mut self,
        every_device: DisplaySettingsContent,
        own: Option<DisplaySettingsContent>,
    ) -> (Uuid, Option<Uuid>) {
        let store = self.test.store();
        store.add_block(BlockInfo::new(
            self.settings,
            Settings::CONTENT_TYPE,
            BlockParent::Root,
        ));
        let shared = Uuid::new_v4();
        let mut settings = SettingsContent::default();
        settings.apply(&Settings::set_kind(DisplaySettings::CONTENT_TYPE, shared));
        store.hold(Some(shared), every_device);
        let own = own.map(|content| {
            let (profile, block) = (Uuid::new_v4(), Uuid::new_v4());
            settings.apply(&Settings::add_profile(self.client, profile));
            let mut layer = SettingsProfileContent::default();
            layer.apply(&SettingsProfile::set_kind(
                DisplaySettings::CONTENT_TYPE,
                block,
            ));
            store.hold(Some(profile), layer);
            store.hold(Some(block), content);
            block
        });
        self.test.hold(Some(self.settings), settings);
        self.settle();
        (shared, own)
    }

    fn report_screens(&mut self, screens: Vec<HostDisplay>) {
        self.test.set_watched::<DisplaysWatch>(&screens);
        self.settle();
    }

    fn with_windows(ids: &[u64]) -> Self {
        let mut fixture = Self::new();
        fixture.settle();
        for focused in std::iter::once(None).chain(ids.iter().copied().map(Some)) {
            let windows = ids
                .iter()
                .map(|id| host_window(*id, focused == Some(*id)))
                .collect();
            fixture.test.set_watched::<WindowsWatch>(&windows);
            fixture.settle();
        }
        fixture.test.clear_calls();
        fixture
    }

    fn focused_windows(&self) -> Vec<HostWindowId> {
        self.test
            .host_calls(block_ui_test::requested::window)
            .into_iter()
            .filter_map(|action| match action {
                WindowAction::Focus(window) => Some(window),
                _ => None,
            })
            .collect()
    }

    fn allow_power(&mut self, power: PowerAvailability) {
        self.test.set_watched::<PowerWatch>(&power);
        self.settle();
    }

    fn wifi(&mut self, status: WifiStatus) {
        self.test.set_watched::<WifiWatch>(&status);
        self.settle();
    }

    fn settle(&mut self) {
        self.test.run();
    }

    fn hear(&mut self, level: f32, muted: bool, default_output: &str) {
        self.test.set_watched::<MediaWatch>(&MediaLevels {
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
                (self.notified, NotificationRequest::Notify(incoming.clone()))
            })
            .collect();
        self.test
            .set_watched::<NotificationsWatch>(&NotificationInbox { requests });
        self.settle();
    }

    fn signals(&mut self) -> Vec<NotificationSignal> {
        self.test
            .take_host_calls(block_ui_test::requested::notification_report)
            .into_iter()
            .flat_map(|report| report.signals)
            .collect()
    }

    fn status_label(&self) -> String {
        let document = self.test.document();
        document
            .find_test_id("desktop.status")
            .and_then(|status| accessible_under(&document, status))
            .unwrap_or_default()
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| text_within(&document, root, words).is_some())
    }

    fn click_text(&mut self, words: &str) {
        let document = self.test.document();
        let node = document
            .root()
            .and_then(|root| text_within(&document, root, words))
            .unwrap_or_else(|| panic!("{words:?} is on screen"));
        let center = document
            .node_rect(node)
            .expect("the text is laid out")
            .center();
        drop(document);
        self.test.click(center);
        self.settle();
    }

    fn on_two_displays(&mut self, left: (Rect, f32), right: (Rect, f32)) {
        self.report_screens(vec![
            monitor("left", left.0, left.1),
            monitor("right", right.0, right.1),
        ]);
        self.test.show_on_displays(&[left, right]);
        self.settle();
    }

    fn desk_text(&self, words: &str) -> Rect {
        let node = {
            let document = self.test.document();
            document
                .root()
                .and_then(|root| text_within(&document, root, words))
                .unwrap_or_else(|| panic!("{words:?} is on screen"))
        };
        self.test
            .desk_rect(node)
            .unwrap_or_else(|| panic!("{words:?} is shown on a display"))
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
                    placement.rect,
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
        drop(document);
        self.test.click(cross.center());
        self.settle();
    }
}

const PANEL: &str = "DPI-1";

fn monitor(id: &str, rect: Rect, scale: f32) -> HostDisplay {
    HostDisplay {
        id: id.to_owned(),
        name: id.to_owned(),
        connector: id.to_owned(),
        rect,
        scale,
        ..epaper_panel()
    }
}

fn epaper_panel() -> HostDisplay {
    let mode = DisplayMode {
        width: 1404,
        height: 1872,
        refresh_millihertz: 85_000,
    };
    HostDisplay {
        id: PANEL.to_owned(),
        name: "E-paper panel".to_owned(),
        connector: PANEL.to_owned(),
        modes: vec![mode],
        default: mode,
        current: mode,
        rotation: Default::default(),
        rect: block_editor_beui::Rect::ZERO,
        scale: 1.0,
    }
}

fn rotate_in_bar() -> DisplaySettingsContent {
    let mut content = DisplaySettingsContent::default();
    content.apply(&DisplaySettings::set_rotate_in_bar(Some(true)));
    content
}

fn network(ssid: &str, strength: u8, security: WifiSecurity, saved: bool) -> WifiNetwork {
    WifiNetwork {
        ssid: ssid.to_owned(),
        strength,
        security,
        saved,
    }
}

fn nearby() -> WifiStatus {
    WifiStatus {
        available: true,
        enabled: true,
        link: WifiLink::Connected("Home".to_owned()),
        networks: vec![
            network("Home", 82, WifiSecurity::Password, true),
            network("Office", 60, WifiSecurity::Unsupported, false),
            network("Cafe", 48, WifiSecurity::Open, false),
            network("Flat", 25, WifiSecurity::Password, false),
        ],
        failed: None,
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
        kind: WindowKind::Window,
        shown: true,
        title: format!("Program {id}"),
        app_id: "test".to_owned(),
        parent: None,
        modal: false,
        size: Vec2::new(320.0, 200.0),
        fullscreen: None,
        responding: true,
        focused,
        icon: None,
        raised: 0,
        origin: Default::default(),
        note: None,
    }
}

fn program_window(id: u64, app_id: &str, title: &str, origin: WindowOrigin) -> HostWindow {
    HostWindow {
        title: title.to_owned(),
        app_id: app_id.to_owned(),
        origin,
        ..host_window(id, false)
    }
}

fn foot(id: u64) -> HostWindow {
    program_window(
        id,
        "foot",
        &format!("Terminal {id}"),
        WindowOrigin {
            program: Some("foot.desktop".to_owned()),
            command: vec!["foot".to_owned()],
            launch: None,
        },
    )
}

fn notes(id: u64) -> HostWindow {
    program_window(
        id,
        "org.example.Notes",
        "Shopping list",
        WindowOrigin {
            program: None,
            command: vec!["/usr/bin/notes".to_owned()],
            launch: None,
        },
    )
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
            wm_class: None,
            icon: None,
        },
        HostProgram {
            id: "foot.desktop".to_owned(),
            name: "Foot".to_owned(),
            generic_name: "Terminal".to_owned(),
            comment: "A fast terminal emulator".to_owned(),
            keywords: vec!["shell".to_owned()],
            wm_class: None,
            icon: Some(HostImage {
                width: SIDE,
                height: SIDE,
                rgba: icon.collect(),
            }),
        },
    ]
}

fn kept_keys(fixture: &Fixture) -> Vec<u64> {
    fixture.kept().iter().map(|kept| kept.key.0).collect()
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
        .find_map(|child| text_within(&document, child, words))
}

fn text_under(document: &Document, id: NodeId) -> Option<String> {
    if let Some(node) = document.arena.kind_of(id) {
        return Some(document.text(node).to_owned());
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_under(&document, child))
}

fn accessible_under(document: &Document, id: NodeId) -> Option<String> {
    if let Some(label) = document.accessibility.get(&id).and_then(|node| node.label()) {
        return Some(label.to_owned());
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| accessible_under(document, child))
}

fn toast(id: u32) -> String {
    format!("toast.{}", (1_u64 << 32) + u64::from(id))
}
