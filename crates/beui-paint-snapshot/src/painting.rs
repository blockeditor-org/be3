use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use image::RgbaImage;

use crate::{BrowserSnapshot, Difference, Snapshot};

/// A painting as its file holds it: what beui painted (`NAME.paint`), or a page rendered as
/// HTML (`NAME.browser.paint`). Whatever reads a painting file decodes it here, so the two
/// kinds are told apart, compared, described and drawn one way.
#[derive(Clone, Debug, PartialEq)]
pub enum Painting {
    Native(Snapshot),
    Browser(BrowserSnapshot),
}

impl Painting {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        match BrowserSnapshot::is_browser(bytes) {
            true => BrowserSnapshot::decode(bytes).map(Self::Browser),
            false => Snapshot::decode(bytes).map(Self::Native),
        }
    }

    /// How many frames it holds; a browser painting is one.
    pub fn frames(&self) -> usize {
        match self {
            Self::Native(snapshot) => snapshot.frames.len(),
            Self::Browser(_) => 1,
        }
    }

    /// Whether both hold frame `index` and it is the same in each.
    pub fn same_frame(&self, other: &Self, index: usize) -> bool {
        match (self, other) {
            (Self::Native(before), Self::Native(after)) => {
                before.frames.get(index).is_some_and(|frame| after.frames.get(index) == Some(frame))
            }
            (Self::Browser(before), Self::Browser(after)) => index == 0 && before == after,
            _ => false,
        }
    }

    /// What changed from `self` to `after`, or None when they are the same painting.
    pub fn difference(&self, after: &Self) -> Option<Difference> {
        match (self, after) {
            (Self::Native(before), Self::Native(after)) => crate::difference(before, after),
            (Self::Browser(before), Self::Browser(after)) if before == after => None,
            (Self::Browser(before), Self::Browser(after)) => Some(Difference {
                description: crate::browser::describe(before, after),
                frame: Some(0),
            }),
            (Self::Native(_), Self::Browser(_)) => Some(Difference {
                description: "the painting became a browser painting".to_owned(),
                frame: None,
            }),
            (Self::Browser(_), Self::Native(_)) => Some(Difference {
                description: "the browser painting became a native painting".to_owned(),
                frame: None,
            }),
        }
    }

    /// Draws frame `index`. A browser painting is drawn by `browser_renderer`, a program
    /// (crates/be-tool-browser-rasterizer) that is handed the painting's file and a PNG to
    /// write.
    pub fn render(
        &self,
        index: usize,
        browser_renderer: Option<&mut Command>,
    ) -> Result<RgbaImage, String> {
        match self {
            Self::Native(snapshot) => crate::render(snapshot, index),
            Self::Browser(_) if index != 0 => Err(format!(
                "a browser painting has only frame 0, there is no frame {index}"
            )),
            Self::Browser(snapshot) => {
                let renderer = browser_renderer
                    .ok_or("a browser painting needs the browser rasterizer, and none was given")?;
                let input = Scratch::new("browser.paint");
                let output = Scratch::new("png");
                std::fs::write(&input.0, snapshot.encode()?).map_err(|error| {
                    format!("could not write {}: {error}", input.0.display())
                })?;
                let result = renderer
                    .arg(&input.0)
                    .arg(&output.0)
                    .output()
                    .map_err(|error| format!("could not start the browser rasterizer: {error}"))?;
                if !result.status.success() {
                    return Err(format!(
                        "browser rasterizer failed: {}",
                        String::from_utf8_lossy(&result.stderr)
                    ));
                }
                image::open(&output.0)
                    .map(|image| image.to_rgba8())
                    .map_err(|error| format!("could not read what the browser rasterizer drew: {error}"))
            }
        }
    }
}

/// A file in the temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(extension: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("be3-painting-{}-{number}.{extension}", std::process::id());
        Self(std::env::temp_dir().join(name))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
