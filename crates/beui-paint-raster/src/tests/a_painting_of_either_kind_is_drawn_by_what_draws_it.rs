use super::*;
use beui_paint_snapshot::{BrowserSnapshot, Painting};

#[test]
fn a_painting_of_either_kind_is_drawn_by_what_draws_it() {
    let native = Painting::Native(triangle([255, 0, 0, 255]));
    let drawn = crate::render_painting(&native, 0, None, None).unwrap();
    assert_eq!(drawn.dimensions(), (8, 8));
    assert!(crate::render_painting(&native, 1, None, None).is_err());

    let page = Painting::Browser(BrowserSnapshot::new([390, 844], "<p>Hello</p>"));
    assert_eq!(
        crate::render_painting(&page, 0, None, None).unwrap_err(),
        "a browser painting needs the browser rasterizer, and none was given"
    );
    assert_eq!(
        crate::render_painting(&page, 1, None, Some(&mut std::process::Command::new("sh")))
            .unwrap_err(),
        "a browser painting has only frame 0, there is no frame 1"
    );
}
