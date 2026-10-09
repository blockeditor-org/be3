use beui::datetime::Date;
use beui::{Document, NodeId, Pos2, Vec2, pos2};
use block_ui_test::DocumentTest;

use crate::{DemoApp, PAGES, Page};

mod a_docked_page_shown_fullscreen_covers_the_dock_until_escape;
mod a_sample_carries_the_source_it_was_written_with;
mod a_sample_shows_its_code_when_asked;
mod every_demo_page_paints_as_accepted;
mod every_demo_page_paints_as_accepted_on_a_phone;
mod holding_alt_and_pressing_q_switches_between_recent_tabs;
mod holding_alt_moves_a_floating_demo_page_by_its_content;
mod paused_edits_wait_for_a_send_and_carry_presence;
mod swiping_back_on_a_phone_shows_the_catalog_behind_the_page;
mod the_code_of_a_sample_can_be_selected;
mod the_demo_leaves_a_pane_saying_nothing_is_open;
mod the_demo_opens_every_page_from_its_catalog;
mod the_demo_pages_scroll_rather_than_spilling_off_a_small_window;
mod the_demo_paints_with_only_the_fonts_beui_carries;
mod the_keep_changes_prompt_shows_on_every_screen;
mod the_keep_changes_sample_keeps_on_enter_and_reverts_on_escape;
mod the_keep_changes_sample_reverts_once_its_countdown_runs_out;
mod the_launcher_sample_narrows_its_programs_as_a_name_is_typed;
mod the_lock_screen_sample_stays_locked_until_its_password_is_typed;
mod the_lock_screen_shows_on_every_screen;
mod the_lock_screens_actions_are_told_apart_by_place;
mod the_overlay_sample_closes_on_escape_and_a_click_outside;
mod the_runs_toggle_shows_where_each_side_typed_and_deleted;

const TODAY: (i32, u8, u8) = (2026, 1, 15);
const WIDE: Vec2 = Vec2::new(1100.0, 800.0);
const REVEAL_TICKS: usize = 40;
const REVEAL_STEP: f32 = 120.0;
const CATALOG_X: f32 = 40.0;

fn demo(size: Vec2) -> DocumentTest {
    let (year, month, day) = TODAY;
    DocumentTest::new(DemoApp::on(Date::new(year, month, day)).document, size)
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
        test.scroll_at(pos2(CATALOG_X, bottom / 2.0), Vec2::new(0.0, -REVEAL_STEP));
    }
    panic!("the catalog never showed {}", page.title());
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
