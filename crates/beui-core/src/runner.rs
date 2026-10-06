use std::error::Error;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use accesskit::TreeUpdate;

use crate::app::accessibility_dump::AccessibilityDump;
use crate::app::{App, SafeArea, Setup, next_batch, safe_rect};
use crate::context::{Context, FrameOutput};
use crate::file_picker::FilePickRequest;
use crate::geometry::Vec2;
use crate::input::{CursorIcon, Event, ImeArea, RawInput};
use crate::renderer::{Loaded, Renderers};

pub struct RunOptions {
    pub title: String,
    pub app_id: Option<String>,
    pub size: Vec2,
    pub accessibility_dump: Option<PathBuf>,
    pub accessibility_tree: bool,
}

impl RunOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            accessibility_dump: None,
            accessibility_tree: false,
        }
    }
}

pub struct Launch {
    pub options: RunOptions,
    pub context: Context,
    pub app: Box<dyn App>,
}

pub type Running = Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>;

pub trait Adapter {
    fn name(&self) -> &'static str;

    fn run(self: Box<Self>, launch: Launch) -> Running;
}

pub trait Platform {
    fn copy(&mut self, text: String);

    fn paste(&mut self) -> Option<String>;

    fn pick_file(&mut self, request: FilePickRequest);

    fn set_cursor(&mut self, _icon: CursorIcon, _touch_emulation: bool) {}

    fn lock_pointer(&mut self, _locked: bool) {}

    fn set_fullscreen(&mut self, _fullscreen: bool) {}

    fn set_handles_back(&mut self, _handles: bool) {}

    fn show_ime(&mut self, _ime: Option<&ImeArea>) {}

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}
}

pub struct Frame {
    pub pending: bool,
    pub deferred: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub close_requested: bool,
}

impl Frame {
    pub fn again(&self) -> bool {
        self.deferred || self.repaint || self.repaint_after.is_zero()
    }
}

#[derive(Default)]
struct Shown {
    cursor_icon: CursorIcon,
    touch_emulation: bool,
    pointer_locked: bool,
    fullscreen: bool,
    handles_back: bool,
}

pub struct Runner {
    app: Box<dyn App>,
    context: Context,
    title: String,
    renderers: Option<Renderers>,
    events: Vec<Event>,
    accessibility: Option<AccessibilityDump>,
    shown: Shown,
    output: Option<FrameOutput>,
    exited: bool,
}

impl Runner {
    pub fn new(launch: Launch) -> Self {
        let Launch {
            options,
            context,
            app,
        } = launch;
        #[cfg(not(target_arch = "wasm32"))]
        let dump = options.accessibility_dump.map(AccessibilityDump::new);
        #[cfg(target_arch = "wasm32")]
        let dump = None;
        let accessibility = dump.or_else(|| {
            options
                .accessibility_tree
                .then(AccessibilityDump::in_memory)
        });
        Self {
            app,
            context,
            title: options.title,
            renderers: None,
            events: Vec::new(),
            accessibility,
            shown: Shown::default(),
            output: None,
            exited: false,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn start(&mut self, loaded: Vec<Loaded>, mut setup: Setup) -> Result<(), Box<dyn Error>> {
        let renderers = Renderers::new(&self.context, loaded)?;
        renderers.provide(&mut setup);
        self.renderers = Some(renderers);
        self.app.setup(&setup);
        Ok(())
    }

    pub fn started(&self) -> bool {
        self.renderers.is_some()
    }

    pub fn renderers(&mut self) -> Option<&mut Renderers> {
        self.renderers.as_mut()
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.output.take()
    }

    pub fn accessibility_text(&self) -> Option<&str> {
        self.accessibility.as_ref().map(AccessibilityDump::text)
    }

    pub fn close_requested(&mut self) -> bool {
        self.app.close_requested()
    }

    pub fn exit(&mut self) {
        if !self.exited {
            self.exited = true;
            self.app.exiting();
        }
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn frame(
        &mut self,
        platform: &mut dyn Platform,
        pixels_per_point: f32,
        safe_area: SafeArea,
    ) -> Option<Frame> {
        let renderers = self.renderers.as_mut()?;
        if let Err(error) = renderers.follow_choice(&self.context) {
            eprintln!("beui: could not switch renderers: {error}");
        }
        let Some(physical) = renderers.physical() else {
            self.events.clear();
            return None;
        };
        self.context.set_pixels_per_point(pixels_per_point);
        let scale = self.context.pixels_per_point();
        let screen = physical / scale;
        let raw = RawInput {
            events: next_batch(&mut self.events),
        };
        let app = &mut self.app;
        let mut output = self.context.run(raw, |context| {
            app.update(context, safe_rect(screen, safe_area, scale));
        });
        let title = &self.title;
        platform.publish_accessibility(&mut || output.accessibility_tree(title, screen));
        if let Some(dump) = &mut self.accessibility {
            dump.update(output.accessibility_tree(title, screen));
        }

        if let Some(text) = &output.copied_text {
            platform.copy(text.clone());
        }
        for request in std::mem::take(&mut output.file_picks) {
            platform.pick_file(request);
        }
        if output.paste_requested
            && let Some(text) = platform.paste()
        {
            self.events.push(Event::Text(text));
        }
        let shown = &mut self.shown;
        if output.pointer_locked != shown.pointer_locked {
            shown.pointer_locked = output.pointer_locked;
            platform.lock_pointer(output.pointer_locked);
        }
        let touch_emulation = self.context.touch_emulation();
        if output.cursor_icon != shown.cursor_icon || touch_emulation != shown.touch_emulation {
            shown.cursor_icon = output.cursor_icon;
            shown.touch_emulation = touch_emulation;
            platform.set_cursor(output.cursor_icon, touch_emulation);
        }
        platform.show_ime(output.ime.as_ref());
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != shown.fullscreen
        {
            shown.fullscreen = fullscreen;
            platform.set_fullscreen(fullscreen);
        }
        if output.handles_back != shown.handles_back {
            shown.handles_back = output.handles_back;
            platform.set_handles_back(output.handles_back);
        }

        let pending = renderers.prepare(&output, scale, self.app.clear_color());
        let frame = Frame {
            pending,
            deferred: !self.events.is_empty(),
            repaint: output.repaint,
            repaint_after: output.repaint_after,
            close_requested: output.close_requested,
        };
        self.output = Some(output);
        Some(frame)
    }

    pub fn present(&mut self) -> bool {
        let background = self.app.clear_color();
        self.renderers
            .as_mut()
            .is_some_and(|renderers| renderers.present(background))
    }
}
