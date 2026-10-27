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
    let mut snapshot = paint_snapshot::BrowserSnapshot::decode(&bytes)?;
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
