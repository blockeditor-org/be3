use std::time::Duration;

use beui::native::{Context, WindowSize};
use beui::vec2;
use beui_host::native::FreetypeFonts;
use beui_host::native::build;
use beui_host::native::testing::{Gui, Showing, Stepped, Target};

use super::*;

mod only_a_click_on_the_content_focuses_its_window;

const SIZE: Vec2 = vec2(640.0, 400.0);

#[derive(Clone, Debug, PartialEq)]
enum Heard {
    Clicked(Pos2),
    Hovered(bool),
    Keyboard(bool),
}

#[derive(Default)]
struct Recorded(RefCell<Vec<Heard>>);

impl ContentInput for Recorded {
    fn clicked(&self, at: Pos2, _panel: Vec2) {
        self.0.borrow_mut().push(Heard::Clicked(at));
    }

    fn hovered(&self, over: bool) {
        self.0.borrow_mut().push(Heard::Hovered(over));
    }

    fn keyboard(&self, held: bool) {
        self.0.borrow_mut().push(Heard::Keyboard(held));
    }
}

struct Shown {
    stepped: Stepped<Showing>,
    heard: Rc<Recorded>,
    keyboard: Trigger,
}

impl Shown {
    fn new() -> Self {
        let heard = Rc::new(Recorded::default());
        let input = heard.clone();
        let mut keyboard = None;
        let document = build(|| {
            let focus = create_trigger();
            keyboard = Some(focus);
            view! {
                <Root>
                    <List direction=beui::Direction::Vertical spacing=Space::Xs>
                        <Interactive @test_id={"grip"}>
                            <Frame height={Length::Size(Size::Custom(40.0))} fill=ColorRole::Control />
                        </Interactive>
                        <CapturedContent
                            input={input as Rc<dyn ContentInput>}
                            focus={Some(focus)}
                            tag={"content".to_owned()}
                        >
                            <Frame height={Length::Size(Size::Custom(300.0))} fill=ColorRole::Accent />
                        </CapturedContent>
                    </List>
                </Root>
            }
        });
        let stepped = Stepped::new(
            Context::new(FreetypeFonts::default()),
            Showing(document),
            WindowSize {
                size: SIZE,
                scale: 1.0,
            },
            None,
        );
        let mut shown = Self {
            stepped,
            heard,
            keyboard: keyboard.expect("the trigger is made while the document is built"),
        };
        shown.settle();
        shown
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.stepped.advance(Duration::from_millis(100));
            self.stepped.frame(Vec::new());
        }
    }

    fn clicks(&self) -> Vec<Pos2> {
        self.heard
            .0
            .borrow()
            .iter()
            .filter_map(|heard| match heard {
                Heard::Clicked(at) => Some(*at),
                Heard::Hovered(_) | Heard::Keyboard(_) => None,
            })
            .collect()
    }

    fn content_point(&self, local: Vec2) -> Target {
        Target::point(self.stepped.rect_of("content").min + local)
    }
}
