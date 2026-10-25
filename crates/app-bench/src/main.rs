use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use be_drive::Driven;
use beui_core::app::automation::Reply;
use paint_snapshot::{Painting, Snapshot};

mod latency;
mod server;

const SIZE: &str = "1100x720";
const ONE_WAY: Duration = Duration::from_millis(15);
const POINTS: usize = 10_000;
const TEXT_BYTES: usize = 50 * 1024 * 1024;
const LONG: Duration = Duration::from_secs(600);
const BLOCK: &str = "#workspace.block";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

struct Settings {
    program: String,
    libraries: Option<String>,
    points: usize,
    text_bytes: usize,
    out: PathBuf,
}

fn setting(name: &str, default: usize) -> Result<usize, String> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("{name} takes a count, not {value}")),
        Err(_) => Ok(default),
    }
}

fn run() -> Result<(), String> {
    let program = std::env::var("BLOCK_APP")
        .map_err(|_| "BLOCK_APP names the block-app to run; run this through //crates/bench:suite (guides/performance.md)".to_owned())?;
    let out = std::env::temp_dir().join(format!("app-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out)
        .map_err(|error| format!("could not create {}: {error}", out.display()))?;
    let settings = Settings {
        program,
        libraries: std::env::var("APP_LIBRARIES").ok(),
        points: setting("APP_BENCH_POINTS", POINTS)?,
        text_bytes: setting("APP_BENCH_TEXT_BYTES", TEXT_BYTES)?,
        out,
    };
    let build_info = Path::new(&settings.program).with_file_name(build_id::FILE);
    let server = server::Server::start(&build_info, settings.out.join("server"))?;
    let proxy = latency::start(server.address, ONE_WAY)
        .map_err(|error| format!("could not start the latency proxy: {error}"))?;
    let url = format!("http://{proxy}");
    let first = first_device(&settings, &server, &url)?;
    let second = second_device(&settings, &server, &url, &first.phrase)?;
    validate(&first, &second)?;
    println!("kept the exports and paintings in {}", settings.out.display());
    Ok(())
}

struct Device {
    phrase: Vec<String>,
    export: Vec<u8>,
    canvas: Snapshot,
    text: Snapshot,
}

fn first_device(settings: &Settings, server: &server::Server, url: &str) -> Result<Device, String> {
    let started = Instant::now();
    let mut app = App::start(settings)?;
    bench::time("first.start", started.elapsed());
    app.add_account(server, url)?;
    let phrase = app.save_phrase()?;
    app.click("Button \"Personal\"")?;
    app.wait("Pane \"Files\"")?;
    app.run(&["settle"])?;
    app.click("Button \"Add to My files\"")?;
    app.click("MenuItem \"Folder\"")?;
    app.wait("TreeItem \"Folder\"")?;
    let folder = app.row_button("TreeItem \"Folder\"", ".more")?;
    app.click(&folder)?;
    app.click("#file-tree.zoom-in")?;
    app.wait("#file-tree.back")?;
    app.run(&["hurry", "off"])?;

    app.add("Canvas", "canvas")?;
    let canvas = app.place("#infinite-canvas.canvas")?;
    let at = |x: f32, y: f32| {
        format!(
            "{:.0},{:.0}",
            canvas.0 + canvas.2 * x,
            canvas.1 + canvas.3 * y
        )
    };
    app.click("#infinite-canvas.dock.tool.Rectangle")?;
    app.run(&["--no-settle", "drag", &at(0.2, 0.2), &at(0.4, 0.4)])?;
    app.save("canvas.rectangle-save")?;
    app.click("#infinite-canvas.dock.tool.Pen")?;
    app.within(LONG, &[
        "--no-settle",
        "pen",
        "draw",
        &at(0.1, 0.6),
        &at(0.9, 0.7),
        &settings.points.to_string(),
    ])?;
    app.save("canvas.line-save")?;
    app.run(&["pen", "leave"])?;
    let canvas = app.paint("canvas.paint", &settings.out.join("first-canvas.paint"))?;
    app.close_tab("Canvas", "canvas.close")?;

    app.add("Text", "text")?;
    app.click("#text.surface")?;
    app.within(LONG, &["clipboard", &text(settings.text_bytes)])?;
    app.within(LONG, &["--no-settle", "key", "ctrl+v"])?;
    app.save("text.paste-and-save")?;
    let text = app.paint("text.paint", &settings.out.join("first-text.paint"))?;
    app.close_tab("Text", "text.close")?;

    let export = app.export("first.export", &settings.out.join("first.block"))?;
    let closing = Instant::now();
    app.close()?;
    bench::time("first.close", closing.elapsed());
    Ok(Device {
        phrase,
        export,
        canvas,
        text,
    })
}

fn second_device(
    settings: &Settings,
    server: &server::Server,
    url: &str,
    phrase: &[String],
) -> Result<Device, String> {
    let started = Instant::now();
    let mut app = App::start(settings)?;
    bench::time("second.start", started.elapsed());
    let signing_in = Instant::now();
    app.add_account(server, url)?;
    bench::time("second.sign-in", signing_in.elapsed());
    let recovering = Instant::now();
    app.recover(phrase)?;
    bench::time("second.recover", recovering.elapsed());
    let opening = Instant::now();
    app.click("Button \"Personal\"")?;
    app.wait("Pane \"Files\"")?;
    app.run(&["settle"])?;
    bench::time("second.open-organization", opening.elapsed());
    bench::time("second.total", started.elapsed());
    app.run(&["hurry", "off"])?;
    if !app.shown("#file-tree.back")? {
        let folder = app.row_button("TreeItem \"Folder\"", ".more")?;
        app.click(&folder)?;
        app.click("#file-tree.zoom-in")?;
        app.wait("#file-tree.back")?;
    }
    let canvas = app.reopen("Canvas", "reopened.canvas", &settings.out.join("second-canvas.paint"))?;
    let text = app.reopen("Text", "reopened.text", &settings.out.join("second-text.paint"))?;
    let export = app.export("second.export", &settings.out.join("second.block"))?;
    app.close()?;
    Ok(Device {
        phrase: phrase.to_vec(),
        export,
        canvas,
        text,
    })
}

fn validate(first: &Device, second: &Device) -> Result<(), String> {
    if first.export.is_empty() {
        return Err("the first export is empty".to_owned());
    }
    if first.export != second.export {
        return Err(format!(
            "the folder exported on the second device ({} bytes) differs from the one exported on the first ({} bytes)",
            second.export.len(),
            first.export.len()
        ));
    }
    if first.canvas != second.canvas {
        return Err("the canvas painted differently on the second device".to_owned());
    }
    if first.text != second.text {
        return Err("the text painted differently on the second device".to_owned());
    }
    Ok(())
}

fn text(bytes: usize) -> String {
    let line = "The quick brown fox jumps over the lazy dog, again and again and again.\n";
    line.repeat(bytes / line.len() + 1)[..bytes].to_owned()
}

struct App {
    driven: Option<Driven>,
}

impl App {
    fn start(settings: &Settings) -> Result<Self, String> {
        let driven = Driven::start(&settings.program, &[], SIZE, settings.libraries.clone())?;
        Ok(Self {
            driven: Some(driven),
        })
    }

    fn driven(&mut self) -> &mut Driven {
        self.driven.as_mut().expect("the app is still open")
    }

    fn within(&mut self, timeout: Duration, words: &[&str]) -> Result<String, String> {
        let driven = self.driven();
        driven.set_timeout(timeout);
        let reply = driven.request(words);
        driven.set_timeout(Duration::from_secs(30));
        match reply {
            Ok(Reply::Text(text)) => Ok(text),
            Ok(Reply::Error(error)) => Err(self.failed(words, &error)),
            Ok(_) => Err(self.failed(words, "answered with something other than text")),
            Err(error) => Err(self.failed(words, &error)),
        }
    }

    fn run(&mut self, words: &[&str]) -> Result<String, String> {
        self.within(Duration::from_secs(30), words)
    }

    fn failed(&mut self, words: &[&str], error: &str) -> String {
        let driven = self.driven();
        let tree = match driven.request(&["tree"]) {
            Ok(Reply::Text(tree)) => tree,
            _ => "(the app did not answer)".to_owned(),
        };
        format!(
            "{} failed: {error}\n\nWhat the app shows:\n{tree}\n\n{}",
            words.join(" "),
            driven.log_report()
        )
    }

    fn wait(&mut self, target: &str) -> Result<(), String> {
        self.run(&["wait", target]).map(drop)
    }

    fn click(&mut self, target: &str) -> Result<(), String> {
        self.wait(target)?;
        self.run(&["click", target]).map(drop)
    }

    fn shown(&mut self, text: &str) -> Result<bool, String> {
        Ok(self.run(&["tree"])?.contains(text))
    }

    fn place(&mut self, target: &str) -> Result<(f32, f32, f32, f32), String> {
        let found = self.run(&["find", target])?;
        let numbers: Vec<f32> = found
            .split(|character: char| {
                !character.is_ascii_digit() && character != '-' && character != '.'
            })
            .filter_map(|number| number.parse().ok())
            .collect();
        match numbers[..] {
            [x, y, width, height, ..] => Ok((x, y, width, height)),
            _ => Err(format!("find answered {found:?}, not a place")),
        }
    }

    fn row_button(&mut self, row: &str, suffix: &str) -> Result<String, String> {
        let tree = self.run(&["tree"])?;
        tree.lines()
            .skip_while(|line| !line.contains(row))
            .take(4)
            .find_map(|line| {
                let id = line.split_whitespace().find(|word| word.starts_with('#'))?;
                id.ends_with(suffix).then(|| id.to_owned())
            })
            .ok_or_else(|| format!("{row} has no button {suffix} in\n{tree}"))
    }

    fn add_account(&mut self, server: &server::Server, url: &str) -> Result<(), String> {
        self.click("Button \"Add account\"")?;
        self.click("#account.custom")?;
        self.click("TextInput \"Server address\"")?;
        self.run(&["type", url])?;
        self.click("#account.confirm")?;
        self.wait("#account.browser-code")?;
        let tree = self.run(&["tree"])?;
        let code = tree
            .lines()
            .find(|line| line.contains("#account.browser-code"))
            .and_then(value)
            .ok_or_else(|| format!("the sign-in shows no code:\n{tree}"))?;
        server.approve(&code)?;
        self.run(&["gone", "#account.browser-code"])?;
        Ok(())
    }

    fn save_phrase(&mut self) -> Result<Vec<String>, String> {
        self.wait("Label value=\"Check that you saved them\"")?;
        let tree = self.run(&["tree"])?;
        let phrase: Vec<String> = tree
            .lines()
            .find(|line| line.contains("value=\"1. "))
            .and_then(value)
            .ok_or_else(|| format!("the recovery phrase is not shown:\n{tree}"))?
            .replace("\\n", " ")
            .split_whitespace()
            .filter(|word| !word.ends_with('.'))
            .map(str::to_owned)
            .collect();
        let asked: Vec<usize> = tree
            .lines()
            .filter_map(|line| {
                let (_, rest) = line.split_once("TextInput \"Word ")?;
                rest.split('"').next()?.parse().ok()
            })
            .collect();
        for position in asked {
            self.click(&format!("TextInput \"Word {position}\""))?;
            self.run(&["type", &phrase[position - 1]])?;
        }
        self.click("Button \"I saved them\"")?;
        Ok(phrase)
    }

    fn recover(&mut self, phrase: &[String]) -> Result<(), String> {
        self.click("#recovery.phrase")?;
        self.run(&["type", &phrase.join(" ")])?;
        self.click("#recovery.confirm")?;
        self.wait("Button \"Personal\"")
    }

    fn add(&mut self, label: &str, metric: &str) -> Result<(), String> {
        self.click("#file-tree.new")?;
        let item = format!("MenuItem \"{label}\"");
        self.wait(&item)?;
        let started = Instant::now();
        self.run(&["--no-settle", "click", &item])?;
        self.wait(&format!("Pane \"{label}\""))?;
        bench::time(&format!("{metric}.tab-open"), started.elapsed());
        self.wait(&format!("TreeItem \"{label}\""))?;
        bench::time(&format!("{metric}.in-files"), started.elapsed());
        self.run(&["gone", "#editor.loading"])?;
        bench::time(&format!("{metric}.loaded"), started.elapsed());
        self.run(&["key", "Enter"])?;
        self.run(&["hurry", "on"])?;
        self.run(&["hurry", "off"]).map(drop)
    }

    fn save(&mut self, metric: &str) -> Result<(), String> {
        let started = Instant::now();
        self.within(LONG, &["hurry", "on"])?;
        bench::time(metric, started.elapsed());
        self.run(&["hurry", "off"]).map(drop)
    }

    fn paint(&mut self, metric: &str, path: &Path) -> Result<Snapshot, String> {
        self.run(&["gone", "#editor.loading"])?;
        self.run(&["settle"])?;
        let started = Instant::now();
        let reply = self.driven().request(&["paint", BLOCK]);
        bench::time(metric, started.elapsed());
        let bytes = match reply {
            Ok(Reply::Painting(bytes)) => bytes,
            Ok(_) => return Err("paint answered with something other than a painting".to_owned()),
            Err(error) => return Err(self.failed(&["paint", BLOCK], &error)),
        };
        std::fs::write(path, &bytes)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        Painting::decode(&bytes).and_then(Snapshot::of_painting)
    }

    fn close_tab(&mut self, label: &str, metric: &str) -> Result<(), String> {
        self.wait("Button \"Close tab\"")?;
        let started = Instant::now();
        self.run(&["--no-settle", "click", "Button \"Close tab\""])?;
        self.run(&["gone", &format!("Pane \"{label}\"")])?;
        bench::time(metric, started.elapsed());
        self.run(&["settle"])?;
        bench::time(&format!("{metric}-settled"), started.elapsed());
        Ok(())
    }

    fn reopen(&mut self, label: &str, metric: &str, painting: &Path) -> Result<Snapshot, String> {
        let row = format!("TreeItem \"{label}\"");
        self.wait(&row)?;
        let started = Instant::now();
        self.run(&["--no-settle", "click", &row])?;
        self.wait(&format!("Pane \"{label}\""))?;
        self.run(&["gone", "#editor.loading"])?;
        bench::time(&format!("{metric}-open"), started.elapsed());
        let painted = self.paint(&format!("{metric}-paint"), painting)?;
        self.close_tab(label, &format!("{metric}-close"))?;
        Ok(painted)
    }

    fn export(&mut self, metric: &str, path: &Path) -> Result<Vec<u8>, String> {
        self.click("#file-tree.back")?;
        let folder = self.row_button("TreeItem \"Folder\"", ".more")?;
        self.click(&folder)?;
        self.click("#file-tree.export")?;
        let item = "MenuItem \"Block file\"";
        self.wait(item)?;
        let started = Instant::now();
        self.within(LONG, &["save", &path.to_string_lossy(), item])?;
        bench::time(metric, started.elapsed());
        let bytes = std::fs::read(path)
            .map_err(|error| format!("could not read the export {}: {error}", path.display()))?;
        bench::count(&format!("{metric}-bytes"), bytes.len() as u64);
        self.click(&folder)?;
        self.click("#file-tree.zoom-in")?;
        self.wait("#file-tree.back")?;
        Ok(bytes)
    }

    fn close(&mut self) -> Result<(), String> {
        let driven = self.driven.take().expect("the app is still open");
        driven.close()
    }
}

fn value(line: &str) -> Option<String> {
    let (_, value) = line.split_once("value=\"")?;
    Some(value.split('"').next()?.to_owned())
}
