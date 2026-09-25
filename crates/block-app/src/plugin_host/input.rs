use beui::{Event, Pos2, Rect, Vec2};
use block_plugin_api::{
    DroppedFile, ImeInput, InputBatch, InputEvent, Message, Modifiers, PointerButton, ScreenId,
    ViewportMetrics, WheelUnit,
};
use std::collections::HashSet;
use uuid::Uuid;

use super::instances::Holes;
use crate::host::{self, Target};

pub(super) struct BlockDragEvent {
    pub(super) position: Vec2,
    pub(super) block_id: Uuid,
    pub(super) block_type: Uuid,
    pub(super) dropped: bool,
}

pub(super) fn block_drag(rect: Rect) -> Option<BlockDragEvent> {
    let payload = host::drag()?;
    let pointer = host::pointer().filter(|position| rect.contains(*position))?;
    if host::claimed(pointer) {
        return None;
    }
    Some(BlockDragEvent {
        position: pointer - rect.min,
        block_id: payload.block_id,
        block_type: payload.block_type,
        dropped: host::drag_released(),
    })
}

pub(super) struct FileDropEvent {
    pub(super) position: Vec2,
    pub(super) files: Vec<DroppedFile>,
    pub(super) dropped: bool,
}

pub(super) fn file_drop(rect: Rect) -> Option<FileDropEvent> {
    let (hovering, dropped) =
        host::input(|input| (input.files_hovered, input.files_dropped.clone()));
    if !hovering && dropped.is_empty() {
        return None;
    }
    let pointer = host::pointer().filter(|position| rect.contains(*position))?;
    if !dropped.is_empty() {
        return Some(FileDropEvent {
            position: pointer - rect.min,
            files: dropped.into_iter().filter_map(read_dropped).collect(),
            dropped: true,
        });
    }
    Some(FileDropEvent {
        position: pointer - rect.min,
        files: Vec::new(),
        dropped: false,
    })
}

fn read_dropped(file: beui::DroppedFile) -> Option<DroppedFile> {
    let name = Some(file.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "File".to_owned());
    let data = match file.bytes {
        Some(bytes) => bytes.to_vec(),
        None => read_path(file.path.as_ref()?)?,
    };
    Some(DroppedFile { name, data })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_path(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[cfg(target_arch = "wasm32")]
fn read_path(_path: &std::path::Path) -> Option<Vec<u8>> {
    None
}

#[derive(Default)]
pub(super) struct InputAdapter {
    captured: bool,
    pointer_inside: bool,
    pressed_buttons: u8,
    focused: bool,
    modifiers: Modifiers,
    over_hole: bool,
    paste_shortcut_down: bool,
    captured_touches: HashSet<(u64, u64)>,
}

impl InputAdapter {
    pub(super) fn focused(&self) -> bool {
        self.focused
    }

    pub(super) fn update(
        &mut self,
        target: Target,
        rect: Rect,
        hovered: bool,
        focused: bool,
        screen: ScreenId,
        holes: &Holes,
    ) -> Vec<Message> {
        self.over_hole =
            !self.captured && host::pointer().is_some_and(|position| holes.contains(position));
        let (events, modifiers) = host::input(|input| (input.events.clone(), input.modifiers));
        let mut normalized = Vec::new();
        if focused != self.focused {
            normalized.push(InputEvent::Focus(focused));
            self.focused = focused;
            if !focused {
                self.captured = false;
                self.pressed_buttons = 0;
                self.captured_touches.clear();
            }
        }

        if rect.width() > 0.0 && rect.height() > 0.0 {
            for event in events {
                self.normalize_event(
                    event,
                    target,
                    rect,
                    hovered,
                    focused,
                    modifiers,
                    holes,
                    &mut normalized,
                );
            }
        }

        let shortcut_down = super::clipboard::paste_shortcut_down();
        let shortcut_pressed = shortcut_down && !self.paste_shortcut_down;
        self.paste_shortcut_down = shortcut_down;
        if focused && shortcut_pressed {
            normalized.push(InputEvent::Paste(String::new()));
        }

        if normalized.is_empty() {
            return Vec::new();
        }
        vec![Message::Input(InputBatch {
            screen,
            events: normalized,
        })]
    }

    fn leave(&mut self, output: &mut Vec<InputEvent>) {
        if self.pointer_inside && !self.captured {
            self.pointer_inside = false;
            output.push(InputEvent::PointerLeft);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn normalize_event(
        &mut self,
        event: Event,
        target: Target,
        rect: Rect,
        hovered: bool,
        focused: bool,
        modifiers: beui::Modifiers,
        holes: &Holes,
        output: &mut Vec<InputEvent>,
    ) {
        let pointer = |position: Pos2, captured: bool| {
            (rect.contains(position)
                && !holes.contains(position)
                && host::reaches(target, position))
                || captured
        };
        match event {
            Event::PointerMoved(position) if pointer(position, self.captured) => {
                self.pointer_inside = true;
                let position = position - rect.min;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            Event::PointerMoved(_) | Event::PointerGone => self.leave(output),
            Event::PointerMotion(delta) if focused => {
                output.push(InputEvent::PointerMotion {
                    x: delta.x,
                    y: delta.y,
                });
            }
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } if pointer(pos, self.captured) => {
                let button_mask = 1 << host::pointer_button_index(button);
                self.pressed_buttons = if pressed {
                    self.pressed_buttons | button_mask
                } else {
                    self.pressed_buttons & !button_mask
                };
                self.captured = self.pressed_buttons != 0;
                let position = pos - rect.min;
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::PointerButton {
                    button: block_ui::input::pointer_button(button),
                    pressed,
                    x: position.x,
                    y: position.y,
                });
                if !pointer(pos, false) {
                    self.leave(output);
                }
            }
            Event::Scroll(delta) if hovered && !self.over_hole => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: WheelUnit::Pixels,
                });
            }
            Event::Zoom(factor) if hovered && !self.over_hole => {
                output.push(InputEvent::Zoom { factor });
            }
            Event::Touch {
                id,
                phase,
                pos,
                force,
            } => {
                let touch = (id.device, id.finger);
                let accepted = match phase {
                    beui::TouchPhase::Start => pointer(pos, false),
                    beui::TouchPhase::Move | beui::TouchPhase::End | beui::TouchPhase::Cancel => {
                        self.captured_touches.contains(&touch)
                    }
                };
                if !accepted {
                    return;
                }
                if phase == beui::TouchPhase::Start {
                    self.captured_touches.insert(touch);
                }
                let position = pos - rect.min;
                output.push(InputEvent::Touch {
                    device: id.device,
                    finger: id.finger,
                    phase: block_ui::input::touch_phase(phase),
                    x: position.x,
                    y: position.y,
                    force,
                });
                if matches!(phase, beui::TouchPhase::End | beui::TouchPhase::Cancel) {
                    self.captured_touches.remove(&touch);
                }
                if phase == beui::TouchPhase::Cancel && self.pressed_buttons & 1 != 0 {
                    self.pressed_buttons &= !1;
                    self.captured = self.pressed_buttons != 0;
                    output.push(InputEvent::PointerButton {
                        button: PointerButton::Primary,
                        pressed: false,
                        x: position.x,
                        y: position.y,
                    });
                }
            }
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } if focused => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Key {
                    key: block_ui::input::protocol_key(key),
                    pressed,
                    repeat,
                });
            }
            Event::Modifiers(modifiers) if focused => {
                push_modifiers(&mut self.modifiers, modifiers, output);
            }
            Event::Text(text) if focused => {
                output.push(InputEvent::Text(text));
            }
            Event::Ime(ime) if focused => {
                output.push(InputEvent::Ime(match ime {
                    beui::ImeEvent::Enabled => ImeInput::Enabled,
                    beui::ImeEvent::Preedit(text) => ImeInput::Preedit(text),
                    beui::ImeEvent::Commit(text) => ImeInput::Commit(text),
                    beui::ImeEvent::Disabled => ImeInput::Disabled,
                }));
            }
            Event::Focus(false) if self.focused => {
                self.focused = false;
                self.captured = false;
                self.pressed_buttons = 0;
                self.captured_touches.clear();
                output.push(InputEvent::Focus(false));
            }
            _ => {}
        }
    }
}

pub(super) fn viewport_metrics(size: Vec2, visible: Rect, scale_factor: f32) -> ViewportMetrics {
    let logical_width = size.x.max(0.0);
    let logical_height = size.y.max(0.0);
    let visible = visible.intersect(Rect::from_min_size(Pos2::ZERO, size));
    ViewportMetrics {
        logical_width,
        logical_height,
        visible_x: visible.min.x.max(0.0),
        visible_y: visible.min.y.max(0.0),
        pixel_width: (visible.width().max(0.0) * scale_factor).round() as u32,
        pixel_height: (visible.height().max(0.0) * scale_factor).round() as u32,
        scale_factor,
    }
}

fn push_modifiers(
    previous: &mut Modifiers,
    modifiers: beui::Modifiers,
    output: &mut Vec<InputEvent>,
) {
    let modifiers = block_ui::input::protocol_modifiers(modifiers);
    if *previous != modifiers {
        *previous = modifiers;
        output.push(InputEvent::Modifiers(modifiers));
    }
}
