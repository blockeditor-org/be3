//! Renders a browser painting (`NAME.browser.paint`, beui-paint-snapshot's `BrowserSnapshot`) to
//! a PNG in Chromium through Playwright, for the pull request's painting comment and
//! `be-tool-local rasterize`. Tests never run it: they compare the painting's bytes.
//!
//! It serves the captured document at `https://snapshot.invalid/` and blocks every request it
//! did not capture, except beui's bundled fonts (`beui_fonts::FACES`), served at the origin's
//! root unless the painting embeds that path. It waits for fonts and images, disables
//! animations, and fails on a missing resource or a failed script rather than drawing an
//! incomplete page.
//!
//! It runs `render.mjs` on Node with Playwright, the same pinned downloads the browser tests run
//! (buck/tools), which `browser_program` (buck/program/drive.bzl) copies into the directory this
//! binary is in: `./scripts/buck run //crates/be-tool-browser-rasterizer:browser-rasterizer --
//! IN OUT`.

use std::io::Write as _;
use std::process::{Command, Stdio};

fn main() {
    if let Err(error) = run() {
        eprintln!("browser rasterization failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [input, output] = arguments.as_slice() else {
        return Err("usage: browser-rasterizer SNAPSHOT.browser.paint OUTPUT.png".into());
    };
    let bytes = std::fs::read(input).map_err(|error| error.to_string())?;
    let mut snapshot = beui_paint_snapshot::BrowserSnapshot::decode(&bytes)?;
    for face in beui_fonts::FACES {
        let url = format!("/{}", face.file);
        if !snapshot.files.contains_key(&url) {
            let bytes = beui_fonts::bytes(face.file).expect("every bundled face has its bytes");
            snapshot = snapshot.file(&url, "font/ttf", bytes);
        }
    }
    let bytes = serde_json::to_vec(&snapshot).map_err(|error| error.to_string())?;
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = exe.parent().ok_or("the rasterizer is in no directory")?;
    let mut child = Command::new(directory.join("node"))
        .arg(directory.join("render.mjs"))
        .env("BE3_CHROME", directory.join("chrome/chrome-headless-shell"))
        .env("LD_LIBRARY_PATH", directory.join("lib"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start Node in {}: {error}", directory.display()))?;
    let written = child.stdin.take().unwrap().write_all(&bytes);
    let result = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into_owned());
    }
    written.map_err(|error| error.to_string())?;
    std::fs::write(output, result.stdout).map_err(|error| error.to_string())
}
