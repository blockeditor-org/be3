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
//! It needs a Python with Playwright (`BE3_PLAYWRIGHT_PYTHON`), set up once with:
//!
//! ```text
//! python3 -m venv ~/.cache/be3/browser-rasterizer
//! ~/.cache/be3/browser-rasterizer/bin/pip install -r crates/be-tool-browser-rasterizer/requirements.txt
//! ~/.cache/be3/browser-rasterizer/bin/python -m playwright install --with-deps chromium
//! export BE3_PLAYWRIGHT_PYTHON=~/.cache/be3/browser-rasterizer/bin/python
//! ```
//!
//! `BE3_CHROME` names a Chrome or Chromium to use instead of Playwright's download, as CI does.

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
    let python = std::env::var_os("BE3_PLAYWRIGHT_PYTHON").unwrap_or_else(|| "python3".into());
    let mut child = Command::new(python)
        .args(["-c", include_str!("render.py")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start Python: {error}"))?;
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
