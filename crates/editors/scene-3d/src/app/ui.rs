use std::cell::Cell;
use std::time::Duration;

use beui::accesskit::{Node, Role};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, Key, KeyPress, NodeId, Vec2};

use crate::camera::Camera;
use crate::renderer::Scene;
use beui::{Align, Direction, ItemSize};

const LONGEST_STEP: f32 = 0.1;
const HINT_INSET: Space = Space::Xl;

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

    fn moving(self) -> bool {
        self.strafe() != 0.0 || self.forward() != 0.0
    }
}

#[component]
pub(crate) fn Scene3DView() -> NodeId {
    let scene = Scene::default();
    let camera = create_signal(Camera::default());
    let walking = store(Cell::new(Walking::default()));
    let clock = store(Cell::new(now()));
    let drawing = create_memo(move || Some(scene.drawing(camera.get())));
    let looking = create_signal(false);
    let idle = create_memo(move || !looking.get());

    let stepping = create_timer(move || {
        let stepped = now();
        let elapsed = stepped
            .saturating_duration_since(clock.with(|clock| clock.replace(stepped)))
            .as_secs_f32()
            .min(LONGEST_STEP);
        let walk = walking.with(Cell::get);
        if !looking.get_untracked() || !walk.moving() {
            return None;
        }
        camera.update(|camera| camera.walk(walk.strafe(), walk.forward(), elapsed));
        Some(Duration::ZERO)
    });

    let look = move |motion: Vec2| {
        camera.update(|camera| camera.look([motion.x, motion.y]));
    };
    let walked = move |press: KeyPress| {
        let mut held = walking.with(Cell::get);
        let handled = held.hold(press.key, press.pressed);
        walking.with(|walking| walking.set(held));
        if held.moving() && !stepping.running() {
            clock.with(|clock| clock.set(now()));
            stepping.start(Duration::ZERO);
        }
        handled
    };
    let released = move |next: bool| {
        if !next {
            walking.with(|walking| walking.set(Walking::default()));
        }
        looking.set(next);
    };

    let described = create_memo(move || {
        let mut node = Node::new(Role::Canvas);
        node.set_label(match looking.get() {
            true => "3D scene, looking around",
            false => "3D scene, click to look around",
        });
        node
    });

    let anchor = NodeRef::new();
    view! {
        <PointerLock
            @node_ref=&anchor
            @test_id={"scene.viewport"}
            locked={Binding::new(looking, released)}
            cursor=CursorIcon::PointingHand
            accessibility={described}
            on_motion={look}
            on_key={walked}
        >|_: PointerLockHandle|
            <List spacing=Space::None>
                <Drawing
                    draw={Prop::dynamic(move || draw_gpu(drawing.get()))}
                    @sizing=ItemSize::Percent(100.0)
                />
                <Floating anchor={anchor} edge=Edge::Bottom open={idle} interactive=false>
                    <Hint />
                </Floating>
            </List>
        </PointerLock>
    }
}

#[component]
fn Hint() -> NodeId {
    view! {
        <Frame padding={Padding::vertical(HINT_INSET)}>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xl>
                    <Shortcut keys="Click" description="look around" />
                    <Shortcut keys="W A S D" description="move" />
                    <Shortcut keys="Esc" description="release" />
                </List>
            </Card>
        </Frame>
    }
}
