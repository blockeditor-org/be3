use block_editor_plugin::{
    EditorHost, EditorRegion, FrameSpec, InputEvent, Instance, Key, PointerButton, Rect, Region,
    pos2, vec2,
};
use std::time::Duration;
use uuid::Uuid;

use crate::app::Scene;

mod clicking_the_scene_grabs_the_cursor_and_escape_releases_it;
mod walking_asks_for_frames_until_the_key_is_let_go;

fn scene() -> (Scene, EditorHost, Region) {
    let host = EditorHost::default();
    let mut scene = Scene::new(host.clone());
    scene.connect(Uuid::new_v4());
    let region = Region {
        region: EditorRegion::Frame,
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)),
        scale_factor: 1.0,
        spec: FrameSpec::default(),
    };
    (scene, host, region)
}

fn click() -> InputEvent {
    InputEvent::PointerButton {
        button: PointerButton::Primary,
        pressed: true,
        x: 400.0,
        y: 300.0,
    }
}

fn key(key: Key, pressed: bool) -> InputEvent {
    InputEvent::Key {
        key,
        pressed,
        repeat: false,
    }
}
