use beui::datetime::Date;
use beui::{Document, Input, NodeId, Pos2, Vec2, pos2};
use block_ui_test::DocumentTest;

use super::*;
use crate::{DemoApp, PAGES, Page};
use beui::icons::ICON_VOLUME_2;
use std::time::Duration;

mod a_card_floats_from_its_tab_and_waits_while_another_tab_is_shown;
mod a_card_keeps_inside_a_small_window;
mod a_card_opened_from_a_button_flips_above_it_near_the_bottom;
mod a_card_opened_from_a_button_opens_under_it_where_there_is_room;
mod a_card_opened_from_a_button_shifts_inside_a_narrow_window;
mod a_card_opened_from_a_button_stands_apart_in_the_light_theme;
mod a_destructive_dialog_does_not_confirm_on_enter;
mod a_dialog_confirms_on_enter_from_its_field;
mod a_dialog_hangs_from_the_tab_that_opened_it;
mod a_dialog_keeps_its_buttons_side_by_side_on_a_narrow_screen;
mod a_docked_page_shown_fullscreen_covers_the_dock_until_escape;
mod a_list_leaves_no_gap_for_a_hidden_frame;
mod a_long_menu_page_scrolls_to_the_row_the_keyboard_is_on;
mod a_long_press_on_the_code_of_a_sample_shows_its_selection_toolbar;
mod a_long_sheet_fades_its_content_and_bounces_as_a_panel;
mod a_menu_page_opens_in_place_of_the_menu_and_its_tree_follows_the_arrow_keys;
mod a_menu_page_opens_in_place_of_the_menu_in_a_sheet;
mod a_menu_widens_to_show_a_long_label_and_its_shortcut;
mod a_modal_dialog_blocks_its_tab_and_stacks_a_nested_one;
mod a_pane_shows_its_tabs_as_icons_across_the_top_or_in_a_narrow_sidebar;
mod a_sample_carries_the_source_it_was_written_with;
mod a_sample_shows_its_code_when_asked;
mod a_scrollbar_shows_over_a_page_as_it_scrolls_and_widens_to_a_track_under_the_pointer;
mod a_sheet_rises_from_the_bottom_on_a_phone;
mod a_table_lines_its_cells_up_under_its_columns;
mod a_tabs_pinned_action_sits_in_its_bar_tiled_and_stacked;
mod a_toast_shows_over_an_open_dialog_and_takes_a_press;
mod a_tooltip_is_as_wide_as_its_label_up_to_a_cap;
mod an_instant_level_display_keeps_its_duration_without_fading;
mod animations_paint_their_progress_and_instant_targets;
mod closing_the_rich_text_tab_disposes_its_editors;
mod dragging_across_the_lines_of_a_long_list_selects_them;
mod editing_a_tree_row_keeps_a_name_on_enter_or_a_click_away_and_escape_puts_it_back;
mod every_demo_page_paints_as_accepted;
mod every_demo_page_paints_as_accepted_on_a_phone;
mod holding_alt_and_pressing_q_switches_between_recent_tabs;
mod holding_alt_moves_a_floating_demo_page_by_its_content;
mod instant_indicators_hold_still_and_resume;
mod paused_edits_wait_for_a_send_and_carry_presence;
mod replacing_a_page_closes_the_dialogs_it_opened;
mod rich_text_styled_on_both_sides_converges_once_sent_both_ways;
mod swiping_back_on_a_phone_shows_the_catalog_behind_the_page;
mod tabbing_through_every_page_shows_each_focus_ring_whole;
mod text_breaks_end_lines_and_breaking_anywhere_fills_them;
mod the_on_screen_keyboard_opens_along_the_foot_of_its_fields_window;
mod the_arrow_keys_move_across_a_menu_grid_and_on_to_the_rows_below;
mod the_catalog_shows_a_page_in_place_of_the_one_shown_unless_ctrl_is_held;
mod the_code_of_a_sample_can_be_selected;
mod the_command_palette_ticks_what_a_menu_ticks;
mod the_crash_notice_sample_reloads_from_its_button;
mod the_demo_leaves_a_pane_saying_nothing_is_open;
mod the_demo_opens_every_page_from_its_catalog;
mod the_demo_pages_scroll_rather_than_spilling_off_a_small_window;
mod the_demo_paints_with_only_the_fonts_beui_carries;
mod the_editor_takes_on_a_theme_chosen_while_it_is_open;
mod the_inspector_opens_and_closes_beside_the_demo_in_the_dom;
mod the_inspector_paints_the_component_shadow_tree;
mod the_inspector_paints_with_the_styled_components;
mod the_keep_changes_sample_keeps_on_enter_and_reverts_on_escape;
mod the_keep_changes_sample_reverts_once_its_countdown_runs_out;
mod the_launcher_sample_narrows_its_programs_as_a_name_is_typed;
mod the_level_display_shows_on_a_change_and_fades_out;
mod the_lock_screen_sample_stays_locked_until_its_password_is_typed;
mod the_lock_screens_actions_are_told_apart_by_place;
mod the_message_toast_sample_offers_its_actions;
mod the_on_screen_keyboard_opens_for_a_field_and_shows_what_its_modes_type;
mod the_overlay_sample_closes_on_escape_and_a_click_outside;
mod the_rich_text_bar_styles_a_selection_and_shows_the_style_under_the_caret;
mod the_rich_text_page_renders_its_blocks_and_voids_and_edits_them_through_them;
mod the_runs_toggle_shows_where_each_side_typed_and_deleted;
mod the_stale_notice_sample_answers_each_of_its_buttons;
mod the_tree_sample_opens_on_the_left_of_a_row_and_its_menu_from_its_button;
mod the_window_sample_floats_beside_its_button_where_nothing_opens_windows;
mod the_window_sample_opens_a_window_of_its_own_where_a_host_opens_windows;
mod toasts_on_the_top_edge_stack_in_the_top_corner;
mod typing_a_fahrenheit_temperature_moves_the_celsius_one;
mod waiting_for_data_shows_a_spinner_until_it_arrives;

const TODAY: (i32, u8, u8) = (2026, 1, 15);
const WIDE: Vec2 = Vec2::new(1100.0, 800.0);
const REVEAL_TICKS: usize = 40;
const REVEAL_STEP: f32 = 120.0;
const CATALOG_X: f32 = 40.0;

fn demo(size: Vec2) -> DocumentTest {
    let (year, month, day) = TODAY;
    DocumentTest::new(DemoApp::on(Date::new(year, month, day)).document, size)
}

fn demo_in(size: Vec2, theme: beui::ThemeKind) -> DocumentTest {
    let (year, month, day) = TODAY;
    let mut test = DocumentTest::new(DemoApp::on(Date::new(year, month, day)).document, size);
    test.context().set_preference_layer(
        beui::PreferenceSource::App,
        beui::PreferenceLayer {
            theme: Some(theme),
            ..beui::PreferenceLayer::EMPTY
        },
    );
    test.frame(Vec::new());
    test
}

fn open_pointing_card(test: &mut DocumentTest) -> (beui::Rect, beui::Rect) {
    reveal(test, "demo.dialog.pointing");
    let button = test.rect_of("demo.dialog.pointing");
    test.click("demo.dialog.pointing");
    test.frame(Vec::new());
    test.settle();
    (button, test.rect_of(&dialog_id(0)))
}

fn alone(size: Vec2, page: Page) -> DocumentTest {
    let (year, month, day) = TODAY;
    DocumentTest::new(
        DemoApp::alone(Date::new(year, month, day), page).document,
        size,
    )
}

fn open(test: &mut DocumentTest, page: Page) {
    let row = format!("demo.catalog.{}", page.title());
    if !test.shows(&row) && test.document().find_test_id("dock.back").is_some() {
        test.click("dock.back");
    }
    for _ in 0..REVEAL_TICKS {
        let bottom = test.size().y;
        let shown = test.shows(&row) && test.rect_of(&row).bottom() <= bottom;
        if shown {
            test.click(&row);
            test.frame(Vec::new());
            return;
        }
        test.scroll(pos2(CATALOG_X, bottom / 2.0), Vec2::new(0.0, REVEAL_STEP));
    }
    panic!("the catalog never showed {}", page.title());
}

fn open_beside(test: &mut DocumentTest, page: Page) {
    test.frame(vec![Input::Modifiers(beui::Modifiers::CTRL)]);
    open(test, page);
    test.frame(vec![Input::Modifiers(beui::Modifiers::NONE)]);
}

fn reveal(test: &mut DocumentTest, test_id: &str) {
    let page = pos2(test.size().x * 0.75, test.size().y / 2.0);
    for _ in 0..REVEAL_TICKS {
        if test.shows(test_id) && test.rect_of(test_id).bottom() <= test.size().y {
            return;
        }
        test.scroll(page, Vec2::new(0.0, REVEAL_STEP));
    }
    panic!("the page never showed {test_id}");
}

fn press_key(test: &mut DocumentTest, key: beui::Key) {
    press_key_with(test, key, beui::Modifiers::NONE);
}

fn press_key_with(test: &mut DocumentTest, key: beui::Key, modifiers: beui::Modifiers) {
    test.key(key, modifiers);
    test.frame(Vec::new());
}

fn select_line(test: &mut DocumentTest, at: Pos2) {
    test.click(at);
    test.frame(Vec::new());
    press_key(test, beui::Key::Home);
    press_key_with(test, beui::Key::End, beui::Modifiers::SHIFT);
}

fn selected_text(test: &mut DocumentTest, region: beui::NodeId) -> String {
    let state = test
        .document()
        .api::<beui::unstyled::SelectableState>(region);
    beui::reactive::with_reactive_scope(test.document_mut(), || state.selected_text())
}

fn focused(test: &DocumentTest, test_id: &str) -> bool {
    let document = test.document();
    document.find_test_id(test_id).is_some()
        && document.focused_node() == document.find_test_id(test_id)
}

fn dialog_id(index: u64) -> String {
    format!("dock.dialog.{}", crate::DIALOG_TABS + index)
}

fn paint_every_page(size: Vec2, suffix: &str) {
    for page in PAGES {
        let name = format!("{}{suffix}", page.title().to_lowercase().replace(' ', "_"));
        let mut test = match page {
            Page::Docking => {
                let mut test = demo(size);
                open(&mut test, page);
                test
            }
            _ => alone(size, page),
        };
        test.snapshot(&name);
    }
}

fn showing(document: &Document, id: NodeId, text: &str) -> usize {
    showing_within(document, id, text, false)
}

fn showing_within(document: &Document, id: NodeId, text: &str, culled: bool) -> usize {
    let laid_out = culled || document.node_rect(id).is_some();
    let here = usize::from(
        laid_out
            && document
                .arena
                .kind_of(id)
                .is_some_and(|node| document.text(node) == text),
    );
    let culled = culled || document.is_culled(id);
    here + document
        .children(id)
        .into_iter()
        .map(|child| showing_within(document, child, text, culled))
        .sum::<usize>()
}

fn lowest_edge(document: &Document, id: NodeId, clip: f32) -> f32 {
    let rect = document.node_rect(id);
    let own = rect.map_or(0.0, |rect| rect.bottom().min(clip));
    let scrolls = document
        .first_offset_within(id)
        .is_some_and(|offset| offset.id() == id);
    let clip = match rect.filter(|_| scrolls) {
        Some(rect) => rect.bottom().min(clip),
        None => clip,
    };
    document
        .children(id)
        .into_iter()
        .fold(own, |lowest, child| {
            lowest.max(lowest_edge(document, child, clip))
        })
}

fn over_the_page(size: Vec2) -> Pos2 {
    pos2(size.x * 0.7, size.y * 0.6)
}

fn level_display() -> DocumentTest {
    let document = build(move || {
        let shown = create_signal(0_u64);
        let level = Some(OsdLevel {
            glyph: ICON_VOLUME_2.to_owned(),
            label: "Volume".to_owned(),
            level: 0.65,
            muted: false,
        });
        view! {
            <Root>
                <List spacing=0.0>
                    <Button
                        @test_id={"test.show"}
                        label="Turn it up"
                        variant=ButtonVariant::Secondary
                        on_click={move || shown.update(|shown| *shown += 1)}
                    />
                    <LevelOsd level={level} shown={shown} />
                </List>
            </Root>
        }
    });
    DocumentTest::new(document, Vec2::new(1100.0, 480.0))
}

fn tab(page: Page) -> u64 {
    Pages::tab_of(page)
        .expect("the page is open in a tab")
        .value()
}
