use std::time::{Duration, Instant};

use block_editor_plugin::be_block::Scene3dContent;
use block_editor_plugin::{
    BlockParent, CursorIcon, EditorHost, Frame, InputEvent, Instance, Key, PaintTarget, Plugin,
    PointerButton, Region,
};
use uuid::Uuid;

use crate::camera::Camera;
use crate::renderer::SceneRenderer;

const LONGEST_STEP: f32 = 0.1;

pub struct Scene3D;

impl Plugin for Scene3D {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(Scene::new(host))
    }
}

#[derive(Clone, Copy, Default)]
struct Walking {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

impl Walking {
    fn hold(&mut self, key: Key, pressed: bool) {
        let held = match key {
            Key::W | Key::ArrowUp => &mut self.forward,
            Key::S | Key::ArrowDown => &mut self.back,
            Key::A | Key::ArrowLeft => &mut self.left,
            Key::D | Key::ArrowRight => &mut self.right,
            _ => return,
        };
        *held = pressed;
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

pub(crate) struct Scene {
    host: EditorHost,
    camera: Camera,
    painted: Option<Camera>,
    walking: Walking,
    looking: bool,
    clock: Instant,
    renderer: Option<SceneRenderer>,
}

impl Scene {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            host,
            camera: Camera::default(),
            painted: None,
            walking: Walking::default(),
            looking: false,
            clock: Instant::now(),
            renderer: None,
        }
    }

    fn look(&mut self, looking: bool) {
        self.looking = looking;
        if !looking {
            self.walking = Walking::default();
        }
        self.host.grab_cursor(looking);
    }
}

impl Instance for Scene {
    fn connect(&mut self, _block_id: Uuid) {}

    fn create_block(&mut self) -> Result<Uuid, String> {
        Ok(self
            .host
            .blocks()
            .create(&Scene3dContent::default(), BlockParent::Detached))
    }

    fn input(&mut self, _region: &Region, event: &InputEvent) {
        match event {
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed: true,
                ..
            } if !self.looking => self.look(true),
            InputEvent::PointerMotion { x, y } if self.looking => self.camera.look([*x, *y]),
            InputEvent::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } if self.looking => self.look(false),
            InputEvent::Key { key, pressed, .. } if self.looking => {
                if !self.walking.moving() {
                    self.clock = Instant::now();
                }
                self.walking.hold(*key, *pressed);
            }
            InputEvent::Focus(false) if self.looking => self.look(false),
            _ => {}
        }
    }

    fn update(&mut self, region: &Region, _settings: Option<&mut Vec<u8>>) -> Frame {
        let now = Instant::now();
        let elapsed = now
            .saturating_duration_since(std::mem::replace(&mut self.clock, now))
            .as_secs_f32()
            .min(LONGEST_STEP);
        if self.walking.moving() {
            self.camera
                .walk(self.walking.strafe(), self.walking.forward(), elapsed);
        }
        Frame {
            changed: self.painted != Some(self.camera),
            repaint_after: self.walking.moving().then_some(Duration::ZERO),
            cursor: match self.looking {
                true => CursorIcon::None,
                false => CursorIcon::Pointer,
            },
            content: None,
            painted: vec![region.rect],
            floating: Vec::new(),
        }
    }

    fn paint(&mut self, target: &PaintTarget<'_>) {
        let renderer = self
            .renderer
            .get_or_insert_with(|| SceneRenderer::new(target.device, target.queue, target.format));
        renderer.paint(target, self.camera);
        self.painted = Some(self.camera);
    }
}
