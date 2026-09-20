use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beui::accesskit::{Node, Role};
use beui::reactive::{
    Align, ClickCatcher, Direction, Focusable, Frame, ItemSize, List, NodeRef, Viewport, clone,
    component, component_accessibility, create_memo, create_signal, view, with_document,
};
use beui::styled::{Card, Shortcut};
use beui::unstyled::{Edge, Floating};
use beui::{CursorIcon, Drawing, Key, KeyPress, NodeId};
use block_editor_plugin::Editor;

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

    let looking = editor.cursor_grabbed();
    let idle = create_memo(clone!(looking -> move || !looking.get()));
    let motion = editor.pointer_motion();
    let focused = editor.input_focused();

    editor.each_frame(clone!(
        editor camera walking looking drawing -> move || {
            let now = Instant::now();
            let elapsed = now
                .saturating_duration_since(clock.replace(now))
                .as_secs_f32()
                .min(LONGEST_STEP);
            let mut next = camera.get();
            if looking.get_untracked() {
                match focused.get_untracked() {
                    true => {
                        let moved = motion.get_untracked();
                        next.look([moved.x, moved.y]);
                        let walk = walking.get();
                        next.walk(walk.strafe(), walk.forward(), elapsed);
                        with_document(|document| {
                            document.request_repaint_after(Duration::ZERO);
                        });
                    }
                    false => {
                        walking.set(Walking::default());
                        editor.grab_cursor(false);
                    }
                }
            }
            if next != camera.get() || drawing.get_untracked().is_none() {
                camera.set(next);
                set_drawing.set(Some(scene.drawing(next)));
            }
        }
    ));

    let grab = clone!(editor -> move || editor.grab_cursor(true));
    let key = clone!(editor walking looking -> move |press: KeyPress| {
        if !looking.get_untracked() {
            return false;
        }
        if press.key == Key::Escape {
            walking.set(Walking::default());
            editor.grab_cursor(false);
            return true;
        }
        let mut held = walking.get();
        let handled = held.hold(press.key, press.pressed);
        walking.set(held);
        handled
    });

    component_accessibility(create_memo(clone!(looking -> move || {
        let mut node = Node::new(Role::Canvas);
        node.set_label(match looking.get() {
            true => "3D scene, looking around",
            false => "3D scene, click to look around",
        });
        node
    })));

    let anchor = NodeRef::new();
    view! {
        <Focusable @node_ref=&anchor on_key={key}>
            <ClickCatcher
                cursor=CursorIcon::PointingHand
                on_click={grab}
                @test_id={"scene.viewport"}
            >
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
            </ClickCatcher>
        </Focusable>
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
