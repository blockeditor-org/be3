//! Renders a browser painting (`NAME.browser.paint`, beui-paint-snapshot's `BrowserSnapshot`) to
//! a PNG in Chromium, for the pull request's painting comment and `be-tool-local rasterize`.
//! Tests never run it: they compare the painting's bytes.
//!
//! It serves the captured document at `https://snapshot.invalid/` and blocks every request it
//! did not capture, except beui's bundled fonts (`beui_fonts::FACES`), served at the origin's
//! root unless the painting embeds that path. It waits for fonts and images, disables
//! animations, and fails on a missing resource or a failed script rather than drawing an
//! incomplete page.
//!
//! It starts the browser tests' pinned headless Chromium (buck/tools) itself and drives it over
//! the DevTools protocol on `--remote-debugging-pipe` (cdp.rs), with the switches, page settings
//! and screenshot steps Playwright used when this ran through it, so the previews draw the same.
//! `browser_program` (buck/program/drive.bzl) lays Chromium and its libraries out beside this
//! binary: `./scripts/buck run //crates/be-tool-browser-rasterizer:browser-rasterizer -- IN OUT`.

mod cdp;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::os::fd::AsRawFd as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use cdp::{Cdp, Events, Sender};

const ORIGIN: &str = "https://snapshot.invalid/";
const STEP: Duration = Duration::from_secs(30);

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
    let mut files = BTreeMap::new();
    for (url, file) in &snapshot.files {
        let body = match (&file.text, &file.bytes) {
            (Some(text), _) => text.concat().into_bytes(),
            (None, Some(bytes)) => bytes.clone(),
            (None, None) => Vec::new(),
        };
        files.insert(without_fragment(url)?, (file.content_type.clone(), body));
    }
    files.insert(
        ORIGIN.to_owned(),
        (
            "text/html; charset=utf-8".to_owned(),
            snapshot.document.concat().into_bytes(),
        ),
    );
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = exe.parent().ok_or("the rasterizer is in no directory")?;
    let mut browser = Browser::launch(directory)?;
    let mut page = Page {
        files,
        missing: BTreeSet::new(),
        errors: Vec::new(),
        failed_sends: Vec::new(),
    };
    let drawn = draw(&mut browser.cdp, &mut page, snapshot.size);
    browser.close();
    let image = drawn.map_err(|error| browser.with_log(error))?;
    let failure = if !page.missing.is_empty() {
        let missing: Vec<_> = page.missing.into_iter().collect();
        Some(format!(
            "Snapshot is missing embedded resources:\n{}",
            missing.join("\n")
        ))
    } else if !image.broken.is_empty() {
        Some(format!(
            "Snapshot images failed to decode:\n{}",
            image.broken.join("\n")
        ))
    } else if !page.failed_sends.is_empty() {
        Some(format!(
            "Could not answer Chromium's requests:\n{}",
            page.failed_sends.join("\n")
        ))
    } else if !page.errors.is_empty() {
        Some(format!(
            "Snapshot scripts failed:\n{}",
            page.errors.join("\n")
        ))
    } else {
        None
    };
    if let Some(failure) = failure {
        return Err(failure);
    }
    std::fs::write(output, image.png).map_err(|error| error.to_string())
}

/// `url` resolved against the origin, without its fragment, as Chromium writes a request's URL.
fn without_fragment(url: &str) -> Result<String, String> {
    let origin = url::Url::parse(ORIGIN).expect("the origin is a URL");
    let mut url = origin
        .join(url)
        .map_err(|error| format!("{url}: {error}"))?;
    url.set_fragment(None);
    Ok(url.into())
}

/// The switches Playwright 1.63 starts its headless shell with (chromiumSwitches.ts and
/// chromium.ts), less those for other browsers, other platforms and windows with a frame.
/// Several change what is drawn (the colour profile, scrollbars, pointer type, the screenshot
/// surface), so the previews keep the pixels they had when they ran through Playwright.
const SWITCHES: &[&str] = &[
    "--disable-field-trial-config",
    "--disable-background-networking",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-back-forward-cache",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-component-update",
    "--no-default-browser-check",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-extensions",
    "--disable-features=AvoidUnnecessaryBeforeUnloadCheckSync,DestroyProfileOnBrowserClose,\
     DialMediaRouteProvider,GlobalMediaControls,HttpsUpgrades,LensOverlay,MediaRouter,PaintHolding,\
     ThirdPartyStoragePartitioning,BlockOriginHeaderModificationOnRedirect,Translate,\
     OptimizationHints",
    "--enable-features=CDPScreenshotNewSurface",
    "--allow-pre-commit-input",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--password-store=basic",
    "--no-service-autorun",
    "--disable-search-engine-choice-screen",
    "--disable-sync",
    "--enable-unsafe-swiftshader",
    "--headless",
    "--hide-scrollbars",
    "--mute-audio",
    "--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4",
    "--no-sandbox",
    "--remote-debugging-pipe",
    "--no-startup-window",
];

/// Chromium, running until [`Browser::close`] or a drop.
struct Browser {
    child: Child,
    cdp: Cdp,
    profile: PathBuf,
    log: Arc<Mutex<Vec<u8>>>,
}

impl Browser {
    /// Starts `directory`'s Chromium with its libraries, on a profile of its own.
    fn launch(directory: &Path) -> Result<Self, String> {
        let profile =
            std::env::temp_dir().join(format!("browser-rasterizer-{}", std::process::id()));
        let (from_us, to_chromium) = std::io::pipe().map_err(|error| error.to_string())?;
        let (from_chromium, to_us) = std::io::pipe().map_err(|error| error.to_string())?;
        let (reads, writes) = (from_us.as_raw_fd(), to_us.as_raw_fd());
        let mut command = Command::new(directory.join("chrome/chrome-headless-shell"));
        command
            .args(SWITCHES)
            .arg(format!("--user-data-dir={}", profile.display()))
            .env("LD_LIBRARY_PATH", directory.join("lib"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        // SAFETY: between fork and exec this calls only fcntl and dup2, which are
        // async-signal-safe, on descriptors the parent keeps open until the spawn returns.
        // unsafe(ffi)
        unsafe {
            command.pre_exec(move || {
                // Moved above 4 first, so neither lands on the other's number before it is read.
                let reads = libc::fcntl(reads, libc::F_DUPFD_CLOEXEC, 10);
                let writes = libc::fcntl(writes, libc::F_DUPFD_CLOEXEC, 10);
                if reads < 0 || writes < 0 || libc::dup2(reads, 3) < 0 || libc::dup2(writes, 4) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().map_err(|error| {
            format!(
                "could not start Chromium in {}: {error}",
                directory.display()
            )
        })?;
        drop((from_us, to_us));
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let written = log.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            while let Ok(read @ 1..) = stderr.read(&mut buffer) {
                written.lock().unwrap().extend_from_slice(&buffer[..read]);
            }
        });
        // Chromium makes the profile directory itself; `Drop` removes it, whatever happens next.
        Ok(Self {
            child,
            cdp: Cdp::new(from_chromium, to_chromium),
            profile,
            log,
        })
    }

    /// Asks Chromium to close, and waits a moment for it before killing it.
    fn close(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let _ = self
            .cdp
            .call(&mut Ignore, None, "Browser.close", json!({}), deadline);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// `error` with what Chromium wrote to its stderr, if anything.
    fn with_log(&mut self, error: String) -> String {
        let error = match (error.as_str(), self.child.try_wait()) {
            ("Chromium closed its pipe", Ok(Some(status))) => {
                format!("Chromium closed its pipe and exited with {status}")
            }
            _ => error,
        };
        let log = self.log.lock().unwrap();
        if log.is_empty() {
            return error;
        }
        format!(
            "{error}\nChromium's log:\n{}",
            String::from_utf8_lossy(&log).trim_end()
        )
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

struct Ignore;

impl Events for Ignore {
    fn event(&mut self, _: &mut Sender, _: Option<&str>, _: &str, _: &Value) {}
}

/// The page's requests, served from the painting, and what went wrong as it loaded.
struct Page {
    files: BTreeMap<String, (String, Vec<u8>)>,
    missing: BTreeSet<String>,
    errors: Vec<String>,
    /// Responses to requests that Chromium did not accept.
    failed_sends: Vec<String>,
}

impl Events for Page {
    fn event(&mut self, cdp: &mut Sender, session: Option<&str>, method: &str, params: &Value) {
        match method {
            "Fetch.requestPaused" => {
                let request = params["requestId"].clone();
                let url = params["request"]["url"].as_str().unwrap_or_default();
                let url = without_fragment(url).unwrap_or_else(|_| url.to_owned());
                let sent = match self.files.get(&url) {
                    Some((content_type, body)) => cdp.send(
                        session,
                        "Fetch.fulfillRequest",
                        json!({
                            "requestId": request,
                            "responseCode": 200,
                            "responseHeaders": [
                                {"name": "content-type", "value": content_type},
                                {"name": "content-length", "value": body.len().to_string()},
                                {"name": "access-control-allow-origin", "value": "*"},
                            ],
                            "body": STANDARD.encode(body),
                        }),
                    ),
                    None => {
                        self.missing.insert(url);
                        cdp.send(
                            session,
                            "Fetch.failRequest",
                            json!({"requestId": request, "errorReason": "Failed"}),
                        )
                    }
                };
                if let Err(error) = sent {
                    self.failed_sends.push(error);
                }
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let description = details["exception"]["description"]
                    .as_str()
                    .or(details["text"].as_str())
                    .unwrap_or("an exception");
                self.errors.push(description.to_owned());
            }
            _ => {}
        }
    }
}

struct Drawn {
    png: Vec<u8>,
    broken: Vec<String>,
}

/// Opens a page `size` CSS pixels big, loads the painting in it, and takes its screenshot.
fn draw(cdp: &mut Cdp, page: &mut Page, size: [u32; 2]) -> Result<Drawn, String> {
    let step = || Instant::now() + STEP;
    let context = cdp.call(
        page,
        None,
        "Target.createBrowserContext",
        json!({"disposeOnDetach": true}),
        step(),
    )?;
    let target = cdp.call(
        page,
        None,
        "Target.createTarget",
        json!({"url": "about:blank", "browserContextId": context["browserContextId"]}),
        step(),
    )?;
    let attached = cdp.call(
        page,
        None,
        "Target.attachToTarget",
        json!({"targetId": target["targetId"], "flatten": true}),
        step(),
    )?;
    let session = attached["sessionId"]
        .as_str()
        .ok_or("no session for the page")?
        .to_owned();
    let session = Some(session.as_str());
    let [width, height] = size;
    // What Playwright's browser context sets on each page, with the rasterizer's options:
    // locale, time zone, a light colour scheme, reduced motion, no service workers, offline.
    let setup = [
        ("Page.enable", json!({})),
        ("Runtime.enable", json!({})),
        ("Network.enable", json!({})),
        ("Network.setCacheDisabled", json!({"cacheDisabled": true})),
        (
            "Fetch.enable",
            json!({"patterns": [{"urlPattern": "*", "requestStage": "Request"}]}),
        ),
        (
            "Network.emulateNetworkConditions",
            json!({"offline": true, "latency": 0, "downloadThroughput": -1, "uploadThroughput": -1}),
        ),
        (
            "Emulation.setFocusEmulationEnabled",
            json!({"enabled": true}),
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({
                "mobile": false,
                "width": width,
                "height": height,
                "screenWidth": width,
                "screenHeight": height,
                "deviceScaleFactor": 1,
                "screenOrientation": {"angle": 0, "type": "landscapePrimary"},
            }),
        ),
        ("Emulation.setLocaleOverride", json!({"locale": "en-US"})),
        (
            "Emulation.setTimezoneOverride",
            json!({"timezoneId": "UTC"}),
        ),
        (
            "Page.setFontFamilies",
            json!({"fontFamilies": {
                "standard": "Times New Roman",
                "fixed": "Monospace",
                "serif": "Times New Roman",
                "sansSerif": "Arial",
                "cursive": "Comic Sans MS",
                "fantasy": "Impact",
            }}),
        ),
        (
            "Emulation.setEmulatedMedia",
            json!({"media": "", "features": [
                {"name": "prefers-color-scheme", "value": "light"},
                {"name": "prefers-reduced-motion", "value": "reduce"},
                {"name": "forced-colors", "value": ""},
                {"name": "prefers-contrast", "value": ""},
            ]}),
        ),
        (
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": "if (navigator.serviceWorker) navigator.serviceWorker.register = async () => {};"}),
        ),
    ];
    for (method, params) in setup {
        cdp.call(page, session, method, params, step())?;
    }
    let navigated = cdp.call(
        page,
        session,
        "Page.navigate",
        json!({"url": ORIGIN}),
        step(),
    )?;
    if let Some(error) = navigated["errorText"].as_str() {
        return Err(format!("could not load the painting: {error}"));
    }
    // The initial about:blank page may have fired its own load event, so ask the document.
    evaluate(
        cdp,
        page,
        session,
        "document.readyState === 'complete' || new Promise(r => addEventListener('load', () => r(null), {once: true}))",
    )?;
    evaluate(cdp, page, session, "document.fonts.ready.then(() => null)")?;
    let broken = evaluate(cdp, page, session, BROKEN_IMAGES)?;
    let broken = broken
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|url| url.as_str().map(str::to_owned))
        .collect();
    evaluate(cdp, page, session, PREPARE_FOR_SCREENSHOT)?;
    evaluate(cdp, page, session, "document.fonts.ready.then(() => null)")?;
    let metrics = cdp.call(page, session, "Page.getLayoutMetrics", json!({}), step())?;
    let viewport = &metrics["visualViewport"];
    let scale = viewport["scale"].as_f64().unwrap_or(1.0);
    let enclosing = |length: u32| (f64::from(length) / scale + 1e-3).floor();
    let clip = json!({
        "x": viewport["pageX"],
        "y": viewport["pageY"],
        "width": enclosing(width),
        "height": enclosing(height),
        "scale": scale,
    });
    let shot = cdp.call(
        page,
        session,
        "Page.captureScreenshot",
        json!({"format": "png", "clip": clip, "captureBeyondViewport": false}),
        step(),
    )?;
    let png = STANDARD
        .decode(shot["data"].as_str().ok_or("the screenshot has no data")?)
        .map_err(|error| error.to_string())?;
    Ok(Drawn { png, broken })
}

/// Runs `expression` in the page, waiting for the promise it returns, and returns its value.
fn evaluate(
    cdp: &mut Cdp,
    page: &mut Page,
    session: Option<&str>,
    expression: &str,
) -> Result<Value, String> {
    let result = cdp.call(
        page,
        session,
        "Runtime.evaluate",
        json!({"expression": expression, "awaitPromise": true, "returnByValue": true}),
        Instant::now() + STEP,
    )?;
    if let Some(details) = result.get("exceptionDetails") {
        let description = details["exception"]["description"]
            .as_str()
            .or(details["text"].as_str());
        return Err(format!(
            "evaluating in the page failed: {}",
            description.unwrap_or("")
        ));
    }
    Ok(result["result"]["value"].clone())
}

/// The URL of each image that does not decode.
const BROKEN_IMAGES: &str = "Promise.all(Array.from(document.images, async (image) => {
    try {
        await image.decode();
        return null;
    } catch {
        return image.currentSrc || image.src;
    }
}))";

/// What Playwright's screenshot does to the page first (`animations: \"disabled\"` and
/// `caret: \"hide\"`): finishes each finite animation and cancels each infinite one, now and as
/// any starts, and hides the caret of every editable element, through every shadow root.
const PREPARE_FOR_SCREENSHOT: &str = "(() => {
    const roots = [];
    const collect = (root) => {
        roots.push(root);
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
        do {
            const node = walker.currentNode;
            const shadow = node instanceof Element ? node.shadowRoot : null;
            if (shadow) collect(shadow);
        } while (walker.nextNode());
    };
    collect(document);
    for (const root of roots) {
        root.querySelectorAll('input,textarea,[contenteditable]').forEach((element) => {
            element.style.setProperty('caret-color', 'transparent', 'important');
        });
    }
    const cancelled = new Set();
    const settle = (root) => {
        for (const animation of root.getAnimations()) {
            if (!animation.effect || animation.playbackRate === 0 || cancelled.has(animation)) continue;
            const end = animation.effect.getComputedTiming().endTime;
            try {
                if (Number.isFinite(end)) {
                    animation.finish();
                } else {
                    animation.cancel();
                    cancelled.add(animation);
                }
            } catch {}
        }
    };
    for (const root of roots) {
        settle(root);
        root.addEventListener('transitionrun', () => settle(root));
        root.addEventListener('animationstart', () => settle(root));
    }
    return null;
})()";
