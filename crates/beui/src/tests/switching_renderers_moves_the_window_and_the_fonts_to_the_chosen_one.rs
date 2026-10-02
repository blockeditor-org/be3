use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;

use super::*;
use beui_core::renderer::{Loaded, Renderer, Renderers, WindowHandle};
use raw_window_handle as rwh;

struct Window;

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        Err(rwh::HandleError::Unavailable)
    }
}

impl rwh::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Err(rwh::HandleError::Unavailable)
    }
}

type Log = Rc<RefCell<Vec<String>>>;

struct Recording {
    name: &'static str,
    log: Log,
}

impl Recording {
    fn record(&self, event: String) {
        self.log.borrow_mut().push(format!("{} {event}", self.name));
    }
}

impl Renderer for Recording {
    fn name(&self) -> &'static str {
        self.name
    }

    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![("Renderer", self.name.to_owned())],
        }
    }

    fn set_active(&mut self, active: bool) {
        self.record(format!("active {active}"));
    }

    fn attach(
        &mut self,
        _window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.record(format!("attach {width}x{height}"));
        Ok(())
    }

    fn detach(&mut self) {
        self.record("detach".to_owned());
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.record(format!("resize {width}x{height}"));
    }

    fn physical(&self) -> Option<Vec2> {
        None
    }

    fn prepare(&mut self, _output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

fn glyphs(context: &Context) -> usize {
    context
        .layout("Aa", FontId::proportional(14.0), TextLayout::DEFAULT)
        .glyphs()
        .len()
}

#[test]
fn switching_renderers_moves_the_window_and_the_fonts_to_the_chosen_one() {
    let log = Log::default();
    let context = Context::new(FreetypeFonts::default());
    let empty = FreetypeFonts::new(FontLibrary::new(FontSources::default()));
    let loaded = vec![
        Loaded {
            renderer: Box::new(Recording {
                name: "first",
                log: log.clone(),
            }),
            fonts: None,
        },
        Loaded {
            renderer: Box::new(Recording {
                name: "second",
                log: log.clone(),
            }),
            fonts: Some(Box::new(empty)),
        },
    ];
    let mut renderers = Renderers::new(&context, loaded).expect("two renderers load");
    renderers
        .attach(Arc::new(Window), 100, 50)
        .expect("the first renderer attaches");
    assert_eq!(
        log.take(),
        [
            "second active false",
            "first active true",
            "first attach 100x50"
        ]
    );
    assert_eq!(context.renderers().names, ["first", "second"]);
    assert_eq!(
        glyphs(&context),
        2,
        "the first renderer shares the context's fonts"
    );

    context.choose_renderer(1);
    assert!(
        renderers
            .follow_choice(&context)
            .expect("the second renderer attaches")
    );
    assert_eq!(
        log.take(),
        [
            "first detach",
            "first active false",
            "second active true",
            "second attach 100x50",
        ]
    );
    assert_eq!(context.renderers().active, 1);
    assert_eq!(
        context.renderer_info().map(|info| info.rows),
        Some(vec![("Renderer", "second".to_owned())])
    );
    assert_eq!(
        glyphs(&context),
        0,
        "the second renderer brings its own fonts"
    );

    renderers.detach();
    context.choose_renderer(0);
    assert!(
        renderers
            .follow_choice(&context)
            .expect("the first renderer resizes")
    );
    assert_eq!(
        log.take(),
        [
            "second detach",
            "second detach",
            "second active false",
            "first active true",
            "first resize 100x50",
        ]
    );
    assert_eq!(glyphs(&context), 2, "the shared fonts come back");
}
