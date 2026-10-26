use super::*;
use crate::{PasteAnswer, PasteKind, Pasted};
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::{Launch, PlatformServices, RunOptions, Runner};
use beui_core::window::WindowState;

type Log = Signal<Vec<String>>;

struct Recording(Log);

impl PlatformServices for Recording {
    fn copy(&mut self, text: String) {
        self.0.update(|log| log.push(format!("copy {text}")));
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        self.0.update(|log| log.push(format!("paste {kinds:?}")));
        answer.send(Some(Pasted::Text("pasted".to_owned())));
    }

    fn pick_file(&mut self, _request: FilePickRequest, _answer: FilePickAnswer) {
        self.0.update(|log| log.push("pick".to_owned()));
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        if window.requests.cursor != shown.requests.cursor {
            self.0
                .update(|log| log.push(format!("cursor {:?}", window.requests.cursor)));
        }
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
        context.accept_pastes(&[PasteKind::Text]);
        context.input(|input| {
            for event in &input.events {
                if let Event::Paste(Pasted::Text(text)) = event {
                    self.typed.update(|typed| typed.push(text.clone()));
                }
            }
        });
        self.frames += 1;
    }
}

#[test]
fn the_runner_tells_the_platform_only_what_changed() {
    let _root = crate::reactive::RootScope::new();
    let log = create_signal(Vec::new());
    let typed = create_signal(Vec::new());
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Typing { frames: 0, typed }),
        layers: Vec::new(),
    });
    let mut platform = Recording(log);
    assert!(
        runner.frame(&mut platform, SafeArea::default()).is_none(),
        "nothing is drawn before the renderers are loaded"
    );
    let loaded = vec![Loaded {
        renderer: Box::new(crate::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");

    let frame = runner
        .frame(&mut platform, SafeArea::default())
        .expect("a loaded runner draws");
    assert_eq!(
        log.update(std::mem::take),
        ["copy copied", "paste [Text]", "cursor Text"]
    );
    assert!(frame.deferred, "the pasted text waits for the next frame");

    runner
        .frame(&mut platform, SafeArea::default())
        .expect("a loaded runner draws");
    assert!(
        log.update(std::mem::take).is_empty(),
        "an unchanged cursor is not set again"
    );
    assert_eq!(typed.update(std::mem::take), ["pasted"]);
}
