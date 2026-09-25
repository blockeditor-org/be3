use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beui::accesskit::{Node, Role};
use beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Viewport, clone, component, create_memo,
    create_signal, view, with_document,
};
use beui::styled::{Card, Shortcut};
use beui::unstyled::{Edge, Floating, PointerLock, PointerLockHandle};
use beui::{CursorIcon, Drawing, Key, KeyPress, NodeId, Vec2};
use block_editor_beui::Editor;

use crate::camera::Camera;
use crate::renderer::Scene;

const LONGEST_STEP: f32 = 0.1;
const HINT_INSET: f32 = 16.0;

#[derive(Clone, Copy, Default)]
struct Walking {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

impl Walking {
    fn hold(&mut self, key: Key, pressed: bool) -> bool {
        let held = match key {
            Key::W | Key::ArrowUp => &mut self.forward,
            Key::S | Key::ArrowDown => &mut self.back,
            Key::A | Key::ArrowLeft => &mut self.left,
            Key::D | Key::ArrowRight => &mut self.right,
            _ => return false,
        };
        *held = pressed;
        true
    }

    fn strafe(self) -> f32 {
        f32::from(self.right) - f32::from(self.left)
    }

    fn forward(self) -> f32 {
        f32::from(self.forward) - f32::from(self.back)
    }
}

#[component]
pub(crate) fn Scene3DView(editor: Editor) -> NodeId {
    let scene = Scene::default();
    let camera = Rc::new(Cell::new(Camera::default()));
    let walking = Rc::new(Cell::new(Walking::default()));
    let clock = Rc::new(Cell::new(Instant::now()));
    let (drawing, set_drawing) = create_signal(None::<Drawing>);
    let (looking, set_looking) = create_signal(false);
    let idle = create_memo(clone!(looking -> move || !looking.get()));

    editor.each_frame(clone!(
        camera walking looking drawing -> move || {
            let now = Instant::now();
            let elapsed = now
                .saturating_duration_since(clock.replace(now))
                .as_secs_f32()
                .min(LONGEST_STEP);
            let mut next = camera.get();
            if looking.get_untracked() {
                let walk = walking.get();
                next.walk(walk.strafe(), walk.forward(), elapsed);
                with_document(|document| document.request_repaint_after(Duration::ZERO));
            }
            if next != camera.get() || drawing.get_untracked().is_none() {
                camera.set(next);
                set_drawing.set(Some(scene.drawing(next)));
            }
        }
    ));

    let look = clone!(camera -> move |motion: Vec2| {
        let mut next = camera.get();
        next.look([motion.x, motion.y]);
        camera.set(next);
    });
    let walked = clone!(walking -> move |press: KeyPress| {
        let mut held = walking.get();
        let handled = held.hold(press.key, press.pressed);
        walking.set(held);
        handled
    });
    let released = clone!(walking set_looking -> move |looking: bool| {
        if !looking {
            walking.set(Walking::default());
        }
        set_looking.set(looking);
    });

    let described = create_memo(clone!(looking -> move || {
        let mut node = Node::new(Role::Canvas);
        node.set_label(match looking.get() {
            true => "3D scene, looking around",
            false => "3D scene, click to look around",
        });
        node
    }));

    let anchor = NodeRef::new();
    view! {
        <PointerLock
            @node_ref=&anchor
            @test_id={"scene.viewport"}
            locked={looking}
            cursor=CursorIcon::PointingHand
            accessibility={described}
            on_change={released}
            on_motion={look}
            on_key={walked}
        >
            {move |_: PointerLockHandle| view! {
                <List spacing=0.0>
                    <Viewport drawing={drawing} @sizing=ItemSize::Percent(100.0) />
                    <Floating
                        anchor={anchor.clone()}
                        edge=Edge::Bottom
                        open={idle}
                        interactive=false
                    >
                        <Hint />
                    </Floating>
                </List>
            }}
        </PointerLock>
    }
}

#[component]
fn Hint() -> NodeId {
    view! {
        <Frame padding_vertical=HINT_INSET>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
                    <Shortcut keys="Click" description="look around" />
                    <Shortcut keys="W A S D" description="move" />
                    <Shortcut keys="Esc" description="release" />
                </List>
            </Card>
        </Frame>
    }
}
