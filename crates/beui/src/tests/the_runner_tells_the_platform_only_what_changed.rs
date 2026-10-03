use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, Platform, RunOptions, Runner};

type Log = Rc<RefCell<Vec<String>>>;

struct Screen;

impl Renderer for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(200.0, 100.0))
    }

    fn prepare(&mut self, output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Recording(Log);

impl Platform for Recording {
    fn copy(&mut self, text: String) {
        self.0.borrow_mut().push(format!("copy {text}"));
    }

    fn paste(&mut self) -> Option<String> {
        self.0.borrow_mut().push("paste".to_owned());
        Some("pasted".to_owned())
    }

    fn pick_file(&mut self, _request: FilePickRequest) {
        self.0.borrow_mut().push("pick".to_owned());
    }

    fn set_cursor(&mut self, icon: CursorIcon, touch_emulation: bool) {
        self.0
            .borrow_mut()
            .push(format!("cursor {icon:?} {touch_emulation}"));
    }
}

struct Typing {
    frames: usize,
    typed: Log,
}

impl App for Typing {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.set_cursor_icon(CursorIcon::Text);
        if self.frames == 0 {
            context.copy_text("copied".to_owned());
            context.request_paste();
        }
        context.input(|input| {
            for event in &input.events {
                if let Event::Text(text) = event {
                    self.typed.borrow_mut().push(text.clone());
                }
            }
        });
        self.frames += 1;
    }
}

#[test]
fn the_runner_tells_the_platform_only_what_changed() {
    let log = Log::default();
    let typed = Log::default();
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Typing {
            frames: 0,
            typed: typed.clone(),
        }),
    });
    let mut platform = Recording(log.clone());
    assert!(
        runner
            .frame(&mut platform, 1.0, SafeArea::default())
            .is_none(),
        "nothing is drawn before the renderers are loaded"
    );
    let loaded = vec![Loaded {
        renderer: Box::new(Screen),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");

    let frame = runner
        .frame(&mut platform, 1.0, SafeArea::default())
        .expect("a loaded runner draws");
    assert_eq!(log.take(), ["copy copied", "paste", "cursor Text false"]);
    assert!(frame.deferred, "the pasted text waits for the next frame");

    runner
        .frame(&mut platform, 1.0, SafeArea::default())
        .expect("a loaded runner draws");
    assert!(
        log.take().is_empty(),
        "an unchanged cursor is not set again"
    );
    assert_eq!(typed.take(), ["pasted"]);
}
