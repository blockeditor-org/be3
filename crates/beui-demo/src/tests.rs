use beui::datetime::Date;
use beui::{Document, NodeId, Pos2, Vec2, pos2};
use block_ui_test::DocumentTest;

use crate::{DemoApp, PAGES, Page};

mod every_demo_page_paints_as_accepted;
mod every_demo_page_paints_as_accepted_on_a_phone;
mod the_demo_leaves_a_pane_saying_nothing_is_open;
mod the_demo_opens_every_page_from_its_catalog;
mod the_demo_pages_scroll_rather_than_spilling_off_a_small_window;
mod the_demo_paints_with_only_the_fonts_beui_carries;

const TODAY: (i32, u8, u8) = (2026, 1, 15);
const WIDE: Vec2 = Vec2::new(1100.0, 800.0);
const REVEAL_TICKS: usize = 40;
const REVEAL_STEP: f32 = 120.0;
const CATALOG_X: f32 = 40.0;

fn demo(size: Vec2) -> DocumentTest {
    let (year, month, day) = TODAY;
    DocumentTest::new(DemoApp::on(Date::new(year, month, day)).document, size)
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
        let mut test = demo(size);
        open(&mut test, page);
        test.snapshot(&format!("{}{suffix}", page.title().to_lowercase()));
    }
}

fn showing(document: &Document, id: NodeId, text: &str) -> usize {
    let laid_out = document.node_rect(id).is_some();
    let here = usize::from(
        laid_out
            && document
                .arena
                .kind_of(id)
                .is_some_and(|node| document.text(node) == text),
    );
    here + document
        .children(id)
        .into_iter()
        .map(|child| showing(document, child, text))
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
