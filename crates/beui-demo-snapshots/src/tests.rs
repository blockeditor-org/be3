#![allow(dead_code)]

include!(env!("BEUI_DEMO"));

use beui::Vec2;
use block_ui_test::DocumentTest;

mod every_demo_page_paints_as_accepted;
mod every_demo_page_paints_as_accepted_on_a_phone;
mod the_demo_paints_with_only_the_fonts_beui_carries;

const TODAY: (i32, u8, u8) = (2026, 1, 15);

fn demo(size: Vec2) -> DocumentTest {
    let (year, month, day) = TODAY;
    DocumentTest::new(DemoApp::on(Date::new(year, month, day)).document, size)
}

fn paint_every_page(size: Vec2, suffix: &str) {
    for page in PAGES {
        let mut test = demo(size);
        let row = format!("demo.catalog.{}", page.title());
        if !test.shows(&row) {
            test.click("dock.back");
        }
        test.click(&row);
        test.frame(Vec::new());
        test.snapshot(&format!("{}{suffix}", page.title().to_lowercase()));
    }
}
