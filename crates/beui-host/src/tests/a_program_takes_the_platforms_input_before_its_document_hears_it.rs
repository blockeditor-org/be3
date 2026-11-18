use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use beui_core::program::SafeArea;
use beui_runner::renderer::Loaded;
use beui_runner::{Incoming, Launch, PlatformServices, RunOptions, Runner, Units};

struct Window;

impl PlatformServices for Window {}

/// Takes every touch, pointer press and change of modifiers for itself, and leaves the rest to
/// its document.
#[derive(Default)]
struct Taking {
    taken: Rc<RefCell<Vec<(WindowId, Input)>>>,
    heard: Rc<RefCell<Vec<Event>>>,
}

impl Program for Taking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let events = context.input(|input| input.events.clone());
        self.heard.borrow_mut().extend(events);
    }

    fn input(&mut self, window: WindowId, input: &Input) -> bool {
        let takes = matches!(
            input,
            Input::Touch { .. } | Input::PointerButton { .. } | Input::Modifiers(_)
        );
        if takes {
            self.taken.borrow_mut().push((window, input.clone()));
        }
        takes
    }
}

#[test]
fn a_program_takes_the_platforms_input_before_its_document_hears_it() {
    let program = Taking::default();
    let (taken, heard) = (Rc::clone(&program.taken), Rc::clone(&program.heard));
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        program: Box::new(program),
        layers: Vec::new(),
    });
    let window_reports = runner.reports().clone();
    runner
        .start(
            Loaded {
                renderer: Box::new(crate::native::testing::Screen(vec2(400.0, 200.0))),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    window_reports.scaled(2.0);
    let mut window = Window;
    runner.frame(&mut window, SafeArea::default());
    heard.borrow_mut().clear();

    // The platform's pixels, at 2 a point.
    let touch = |phase| Input::Touch {
        id: TouchId {
            device: 1,
            finger: 0,
        },
        phase,
        pos: pos2(100.0, 60.0),
        force: None,
    };
    runner.push(touch(TouchPhase::Start));
    runner.push(Input::PointerButton {
        pos: pos2(40.0, 20.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    runner.push(Input::Text("x".to_owned()));
    runner.push(Incoming::Input {
        units: Units::Points,
        window: WindowId::MAIN,
        input: touch(TouchPhase::End),
    });
    runner.frame(&mut window, SafeArea::default());

    let points = |phase| Input::Touch {
        id: TouchId {
            device: 1,
            finger: 0,
        },
        phase,
        pos: pos2(50.0, 30.0),
        force: None,
    };
    assert_eq!(
        *taken.borrow(),
        vec![
            (WindowId::MAIN, points(TouchPhase::Start)),
            (
                WindowId::MAIN,
                Input::PointerButton {
                    pos: pos2(20.0, 10.0),
                    button: PointerButton::Primary,
                    pressed: true,
                },
            ),
            (
                WindowId::MAIN,
                Input::Touch {
                    id: TouchId {
                        device: 1,
                        finger: 0,
                    },
                    phase: TouchPhase::End,
                    pos: pos2(100.0, 60.0),
                    force: None,
                },
            ),
        ],
        "the program takes what it wants in its window's points, whatever units it came in, in order"
    );
    taken.borrow_mut().clear();
    runner.push(Input::Modifiers(Modifiers::SHIFT));
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        runner.modifiers(),
        Modifiers::SHIFT,
        "the intake still follows the modifiers the program took, for the input it leaves"
    );
    let heard = heard.borrow();
    assert!(
        heard.contains(&Event::Text("x".to_owned())),
        "what it leaves reaches its document: {heard:?}"
    );
    assert!(
        !heard
            .iter()
            .any(|event| matches!(event, Event::Touch { .. } | Event::PointerButton { .. })),
        "what it takes its document never hears: {heard:?}"
    );
}
