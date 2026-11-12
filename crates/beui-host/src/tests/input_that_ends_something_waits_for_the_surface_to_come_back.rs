use super::*;
use beui_core::program::SafeArea;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, PlatformServices, RunOptions, Runner};

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Heard(Rc<RefCell<Vec<Event>>>);

impl Program for Heard {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let events = context.input(|input| input.events.clone());
        self.0.borrow_mut().extend(events);
    }
}

fn touch(phase: TouchPhase) -> Input {
    Input::Touch {
        id: TouchId {
            device: 0,
            finger: 1,
        },
        phase,
        pos: pos2(20.0, 20.0),
        force: None,
    }
}

#[test]
fn input_that_ends_something_waits_for_the_surface_to_come_back() {
    let _root = crate::reactive::RootScope::new();
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        program: Box::new(Heard(heard.clone())),
        layers: Vec::new(),
    });
    runner
        .start(
            Loaded {
                renderer: Box::new(crate::native::testing::Screen(vec2(200.0, 100.0))),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    runner.frame(&mut Nowhere, SafeArea::default());
    runner.push(touch(TouchPhase::Start));
    runner.frame(&mut Nowhere, SafeArea::default());
    heard.borrow_mut().clear();

    runner.reports().resized(0, 0);
    runner.push(touch(TouchPhase::Move));
    runner.push(touch(TouchPhase::Cancel));
    runner.push(Input::Focus(false));
    runner.push(Input::Ime(ImeEvent::FinishComposing));
    runner.push(Input::Back(beui::BackGesture::Invoked));
    runner.frame(&mut Nowhere, SafeArea::default());
    assert!(
        heard.borrow().is_empty(),
        "nothing is heard while there is no surface"
    );
    assert!(
        !runner.has_events(),
        "what is kept does not keep the platform busy while it waits"
    );

    runner.reports().resized(200, 100);
    runner.frame(&mut Nowhere, SafeArea::default());
    let heard = heard.take();
    assert!(
        heard.iter().any(|event| matches!(
            event,
            Event::Touch {
                phase: TouchPhase::Cancel,
                ..
            }
        )),
        "the touch is cancelled once the surface is back: {heard:?}"
    );
    assert!(
        !heard.iter().any(|event| matches!(
            event,
            Event::Touch {
                phase: TouchPhase::Move,
                ..
            }
        )),
        "a move made while there was no surface is dropped: {heard:?}"
    );
    assert!(heard.contains(&Event::Focus(false)), "{heard:?}");
    assert!(
        heard.contains(&Event::Ime(ImeEvent::FinishComposing)),
        "{heard:?}"
    );
    assert!(
        !heard.iter().any(|event| matches!(event, Event::Back(_))),
        "a back gesture invoked over nothing shown does not go back: {heard:?}"
    );
}
