use std::process::Command;

use super::*;
use crate::{BrowserSnapshot, Painting};

#[test]
fn a_browser_painting_is_drawn_by_the_renderer_it_is_given() {
    let native = Painting::Native(triangle([255, 0, 0, 255]));
    let drawn = native.render(0, None, None).unwrap();
    assert_eq!(drawn.dimensions(), (8, 8));
    assert!(native.render(1, None, None).is_err());

    let page = Painting::Browser(BrowserSnapshot::new([390, 844], "<p>Hello</p>"));
    assert_eq!(
        page.render(0, None, None).unwrap_err(),
        "a browser painting needs the browser rasterizer, and none was given"
    );

    // A renderer that checks it was handed the painting and draws the native one.
    let directory = std::env::temp_dir().join(format!(
        "be3-painting-renderer-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let picture = directory.join("drawn.png");
    drawn.save(&picture).unwrap();
    let mut renderer = Command::new("sh");
    renderer.args([
        "-c",
        "grep -q Hello \"$1\" && cp \"$0\" \"$2\"",
        picture.to_str().unwrap(),
    ]);
    assert_eq!(page.render(0, None, Some(&mut renderer)).unwrap(), drawn);

    // Given the file it was read from, the renderer is handed that file as it is.
    let file = directory.join("page.browser.paint");
    std::fs::write(&file, b"Hello from the original file").unwrap();
    let mut renderer = Command::new("sh");
    renderer.args([
        "-c",
        "[ \"$1\" = \"$2\" ] && cp \"$0\" \"$3\"",
        picture.to_str().unwrap(),
    ]);
    renderer.arg(&file);
    assert_eq!(page.render(0, Some(&file), Some(&mut renderer)).unwrap(), drawn);
    assert_eq!(
        page.render(1, None, Some(&mut Command::new("sh"))).unwrap_err(),
        "a browser painting has only frame 0, there is no frame 1"
    );

    let mut failing = Command::new("sh");
    failing.args(["-c", "echo no browser >&2; exit 1"]);
    assert_eq!(
        page.render(0, None, Some(&mut failing)).unwrap_err(),
        "browser rasterizer failed: no browser\n"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
