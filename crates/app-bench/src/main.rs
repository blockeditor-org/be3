use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use be_block::BlockContent;
use be_drive::Driven;
use beui_layer_drive::Reply;
use infinite_canvas_types::{CanvasContent, CanvasEntity, CanvasShape};
use paint_snapshot::{Painting, Snapshot};

mod cores;
mod server;

const SIZE: &str = "1100x720";
const LATENCY: &str = "30";
const POINTS: usize = 10_000;
const STRETCH: usize = 400;
const STYLUS_PER_FRAME: usize = 4;
const BULK_PER_FRAME: usize = 1_000;
const CANVAS_PLUGIN: &str = "be3.infinite-canvas";
const TEXT_BYTES: usize = 50 * 1024 * 1024;
const LONG: Duration = Duration::from_secs(600);
const BLOCK: &str = "#workspace.block";
const SHAPE: &str = "infinite-canvas.entity.";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let ran = match arguments.as_slice() {
        [serve, build_info, data] if serve == server::SERVE => {
            server::serve(Path::new(build_info), Path::new(data))
        }
        _ => {
            // unsafe(env)
            unsafe { std::env::set_var("BEUI_RENDERER", "none") };
            // unsafe(env)
            unsafe { std::env::set_var("BE_LATENCY", LATENCY) };
            run()
        }
    };
    match ran {
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
    let measured = measure(&settings);
    match &measured {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&settings.out);
        }
        Err(_) => eprintln!(
            "kept the server, exports and paintings in {}",
            settings.out.display()
        ),
    }
    measured
}

fn measure(settings: &Settings) -> Result<(), String> {
    let build_info = Path::new(&settings.program).with_file_name(build_id::FILE);
    let cores = cores::Cores::split()?;
    eprintln!("{}", cores.describe());
    let server = server::Server::start(&build_info, &settings.out.join("server"), &cores)?;
    cores.keep_to_app()?;
    let url = server.url.clone();
    let (first, stroke) = first_device(settings, &server, &url)?;
    let second = second_device(settings, &server, &url, &first.phrase)?;
    validate(&first, &second)?;
    stroke.check(&first.export)
}

struct Device {
    phrase: Vec<String>,
    export: Vec<u8>,
    canvas: Snapshot,
    text: Snapshot,
}

fn first_device(
    settings: &Settings,
    server: &server::Server,
    url: &str,
) -> Result<(Device, Stroke), String> {
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
    app.wait("TextInput \"Name\" value=\"Folder\" focused")?;
    app.run(&["key", "Enter"])?;
    app.run(&["gone", "TextInput \"Name\""])?;
    let folder = app.row_button("TreeItem \"Folder\"", ".more")?;
    app.click(&folder)?;
    app.click("#file-tree.zoom-in")?;
    app.wait("#file-tree.back")?;
    app.click("Button \"Close tab\"")?;
    app.run(&["gone", "Tab \"Folder\""])?;
    app.run(&["hurry", "off"])?;

    app.add("Canvas", "canvas")?;
    let canvas = app.place("#infinite-canvas.canvas")?;
    let at = |x: f32, y: f32| (canvas.0 + canvas.2 * x, canvas.1 + canvas.3 * y);
    app.click("#infinite-canvas.dock.tool.Rectangle")?;
    app.run(&[
        "--no-settle",
        "drag",
        &target(at(0.2, 0.2)),
        &target(at(0.4, 0.4)),
    ])?;
    app.wait(SHAPE)?;
    app.save("canvas.rectangle-save")?;
    app.click("#infinite-canvas.dock.tool.Pen")?;
    let stroke = draw_stroke(&mut app, settings.points, at(0.1, 0.6), at(0.9, 0.7))?;
    app.run(&["pen", "leave"])?;
    app.click("#infinite-canvas.dock.tool.Select")?;
    app.paint("canvas.paint", &settings.out.join("first-canvas.paint"))?;
    app.close_tab("Canvas", "canvas.close")?;

    app.add("Text", "text")?;
    app.click("#text.surface")?;
    app.within(LONG, &["clipboard", &text(settings.text_bytes)])?;
    let pasting = Instant::now();
    app.within(LONG, &["--no-settle", "key", "ctrl+v"])?;
    app.within(LONG, &["wait", "The quick brown fox"])?;
    bench::time("text.paste", pasting.elapsed());
    app.save("text.paste-save")?;
    app.paint("text.paint", &settings.out.join("first-text.paint"))?;
    app.close_tab("Text", "text.close")?;

    let export = app.export("first.export", &settings.out.join("first.block"))?;
    let canvas = app.reopen(
        "TreeItem \"Canvas\"",
        "Canvas",
        "first.reopened.canvas",
        &settings.out.join("first-reopened-canvas.paint"),
    )?;
    let text = app.reopen(
        "TreeItem \"The quick brown fox",
        "Text",
        "first.reopened.text",
        &settings.out.join("first-reopened-text.paint"),
    )?;
    let closing = Instant::now();
    app.close()?;
    bench::time("first.close", closing.elapsed());
    Ok((
        Device {
            phrase,
            export,
            canvas,
            text,
        },
        stroke,
    ))
}

fn draw_stroke(
    app: &mut App,
    points: usize,
    from: (f32, f32),
    to: (f32, f32),
) -> Result<Stroke, String> {
    let stretch = STRETCH.min(points / 3).max(1);
    let along = |sample: usize| {
        let fraction = sample as f32 / points as f32;
        (
            from.0 + (to.0 - from.0) * fraction,
            from.1 + (to.1 - from.1) * fraction,
        )
    };
    let stretches = [
        (
            Some("canvas.stroke-start"),
            along(stretch),
            stretch,
            STYLUS_PER_FRAME,
        ),
        (
            None,
            along(points - stretch),
            points - 2 * stretch,
            BULK_PER_FRAME,
        ),
        (Some("canvas.stroke-end"), to, stretch, STYLUS_PER_FRAME),
    ];
    let from = target(from);
    let stretches =
        stretches.map(|(metric, to, steps, per_frame)| (metric, target(to), steps, per_frame));
    let stroke = Stroke::sent(
        &from,
        &stretches
            .each_ref()
            .map(|(_, to, steps, _)| (to.as_str(), *steps)),
        app.scale()?,
    )?;
    app.run(&["--no-settle", "pen", "down", &from])?;
    for (metric, to, steps, per_frame) in &stretches {
        app.stretch(*metric, to, *steps, *per_frame)?;
    }
    app.run(&["frames"])?;
    app.run(&["--no-settle", "pen", "up"])?;
    app.save("canvas.stroke-save")?;
    let frames = app.run(&["frames"])?;
    bench::time(
        "canvas.stroke-lift-longest-plugin-frame",
        longest(&frames, CANVAS_PLUGIN)?,
    );
    Ok(stroke)
}

fn target((x, y): (f32, f32)) -> String {
    format!("{x:.2},{y:.2}")
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
    app.within(LONG, &["wait", "Pane \"Files\""])?;
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
    let canvas = app.reopen(
        "TreeItem \"Canvas\"",
        "Canvas",
        "second.reopened.canvas",
        &settings.out.join("second-reopened-canvas.paint"),
    )?;
    let text = app.reopen(
        "TreeItem \"The quick brown fox",
        "Text",
        "second.reopened.text",
        &settings.out.join("second-reopened-text.paint"),
    )?;
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
        let driven = Driven::start(
            &settings.program,
            &[],
            SIZE,
            settings.libraries.clone(),
            false,
        )?;
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
        let unread: Vec<&str> = std::iter::once("--changes=0")
            .chain(words.iter().copied())
            .collect();
        let reply = driven.request(&unread);
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

    fn stretch(
        &mut self,
        metric: Option<&str>,
        to: &str,
        steps: usize,
        per_frame: usize,
    ) -> Result<(), String> {
        let (x, y) = to
            .split_once(',')
            .and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))
            .ok_or_else(|| format!("{to} is not a point"))?;
        let inked = target((x - 2.0, y));
        self.run(&["frames"])?;
        self.within(
            LONG,
            &[
                "--no-settle",
                "pen",
                "move",
                to,
                "0.5",
                &steps.to_string(),
                &per_frame.to_string(),
            ],
        )?;
        let moved = Instant::now();
        self.within(LONG, &["wait", SHAPE, "over", &inked])?;
        let Some(metric) = metric else {
            return Ok(());
        };
        bench::time(&format!("{metric}-catch-up"), moved.elapsed());
        let frames = self.run(&["frames"])?;
        bench::time(
            &format!("{metric}-longest-frame"),
            longest(&frames, "window")?,
        );
        bench::time(
            &format!("{metric}-longest-plugin-frame"),
            longest(&frames, CANVAS_PLUGIN)?,
        );
        Ok(())
    }

    fn scale(&mut self) -> Result<f32, String> {
        let state = self.run(&["state"])?;
        state
            .split_once(" at scale ")
            .and_then(|(_, rest)| rest.split(',').next()?.parse().ok())
            .ok_or_else(|| format!("the state names no scale:\n{state}"))
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
            .take(6)
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
        self.click("TextInput \"Recovery phrase\"")?;
        self.run(&["type", &phrase.join(" ")])?;
        self.click("Button \"Unlock\"")?;
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
        let snapshot = Painting::decode(&bytes).and_then(Snapshot::of_painting)?;
        std::fs::write(path, snapshot.encode()?)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        Ok(snapshot)
    }

    fn close_tab(&mut self, label: &str, metric: &str) -> Result<(), String> {
        self.wait("Button \"Close tab\"")?;
        let started = Instant::now();
        self.run(&["--no-settle", "click", "Button \"Close tab\""])?;
        self.run(&["gone", &format!("Pane \"{label}\"")])?;
        bench::time(metric, started.elapsed());
        Ok(())
    }

    fn reopen(
        &mut self,
        row: &str,
        label: &str,
        metric: &str,
        painting: &Path,
    ) -> Result<Snapshot, String> {
        self.wait(row)?;
        let started = Instant::now();
        self.run(&["--no-settle", "click", row])?;
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
        self.wait("#file-tree.export")?;
        let started = Instant::now();
        self.within(
            LONG,
            &["save", &path.to_string_lossy(), "#file-tree.export"],
        )?;
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

fn longest(frames: &str, name: &str) -> Result<Duration, String> {
    frames
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(": ")?;
            let (_, longest) = rest.split_once(", the longest ")?;
            longest.strip_suffix(" ms")?.parse::<f64>().ok()
        })
        .map(|milliseconds| Duration::from_secs_f64(milliseconds / 1000.0))
        .ok_or_else(|| format!("frames names no frames of {name}:\n{frames}"))
}

struct Stroke {
    kept: Vec<(f32, f32)>,
    sent: usize,
    spacing: f32,
}

impl Stroke {
    fn sent(from: &str, stretches: &[(&str, usize)], scale: f32) -> Result<Self, String> {
        let place = |target: &str| -> Result<(f32, f32), String> {
            let (x, y) = target
                .split_once(',')
                .ok_or_else(|| format!("{target} is not a point"))?;
            let number = |text: &str| {
                text.parse::<f32>()
                    .map(|pixels| pixels / scale)
                    .map_err(|_| format!("{target} is not a point"))
            };
            Ok((number(x)?, number(y)?))
        };
        let first = place(from)?;
        let mut kept = vec![first];
        let mut at = first;
        let mut sent = 1;
        for (to, steps) in stretches {
            let (start, end) = (at, place(to)?);
            for step in 1..=*steps {
                let fraction = step as f32 / *steps as f32;
                let next = (
                    start.0 + (end.0 - start.0) * fraction,
                    start.1 + (end.1 - start.1) * fraction,
                );
                let last = kept[kept.len() - 1];
                if (next.0 - last.0).hypot(next.1 - last.1) > 1.0 {
                    kept.push(next);
                }
            }
            sent += steps;
            at = end;
        }
        Ok(Self {
            kept,
            sent,
            spacing: (at.0 - first.0).hypot(at.1 - first.1) / (sent - 1).max(1) as f32,
        })
    }

    fn check(&self, export: &[u8]) -> Result<(), String> {
        let drawn = drawn_stroke(export)?;
        if drawn.len() != self.kept.len() {
            return Err(format!(
                "the stroke holds {} points, but the canvas keeps {} of the {} samples drawn: each one more than a point from the last it kept",
                drawn.len(),
                self.kept.len(),
                self.sent
            ));
        }
        bench::count("canvas.line-points", drawn.len() as u64);
        let (first, last) = (self.kept[0], self.kept[self.kept.len() - 1]);
        let (start, end) = (drawn[0], drawn[drawn.len() - 1]);
        let scale = (end.0 - start.0) / (last.0 - first.0);
        let reach = scale * self.spacing / 2.0;
        for (index, (kept, drawn)) in self.kept.iter().zip(&drawn).enumerate() {
            let expected = (
                start.0 + (kept.0 - first.0) * scale,
                start.1 + (kept.1 - first.1) * scale,
            );
            if (drawn.0 - expected.0).hypot(drawn.1 - expected.1) > reach {
                return Err(format!(
                    "point {index} of the stroke is at {drawn:?}, not at {expected:?} where the sample the canvas should have kept was"
                ));
            }
        }
        Ok(())
    }
}

fn drawn_stroke(export: &[u8]) -> Result<Vec<(f32, f32)>, String> {
    let blocks = be_client::transfer::decode(export)?;
    let content = blocks
        .iter()
        .filter(|block| block.block_type == <CanvasContent as BlockContent>::CONTENT_TYPE)
        .find_map(|block| block.content.as_deref())
        .ok_or("the export holds no canvas")?;
    let canvas = CanvasContent::decode(content)
        .map_err(|error| format!("the exported canvas does not decode: {error:?}"))?;
    let strokes: Vec<CanvasEntity> = canvas
        .root()
        .entities()
        .into_iter()
        .filter(|entity| entity.shape == CanvasShape::Freehand)
        .collect();
    let [stroke] = &strokes[..] else {
        return Err(format!(
            "the exported canvas holds {} strokes, not one",
            strokes.len()
        ));
    };
    if stroke.pressures.len() != stroke.points.len() {
        return Err(format!(
            "the stroke holds {} points but {} pressures",
            stroke.points.len(),
            stroke.pressures.len()
        ));
    }
    Ok(stroke
        .world_points()
        .iter()
        .map(|point| (point.x, point.y))
        .collect())
}

fn value(line: &str) -> Option<String> {
    let (_, value) = line.split_once("value=\"")?;
    Some(value.split('"').next()?.to_owned())
}
