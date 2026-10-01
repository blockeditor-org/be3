use beui::{Event, Pos2, Rect, Vec2};
use block_plugin_api::{
    DroppedFile, ImeInput, InputBatch, InputEvent, Message, Modifiers, PointerButton, ScreenId,
    ViewportMetrics, WheelUnit,
};
use std::collections::HashSet;
use uuid::Uuid;

use crate::host;

pub(super) struct BlockDragEvent {
    pub(super) position: Vec2,
    pub(super) block_id: Uuid,
    pub(super) block_type: Uuid,
    pub(super) dropped: bool,
}

pub(super) fn block_drag(input: &beui::ForwardedInput) -> Option<BlockDragEvent> {
    let payload = host::drag()?;
    let pointer = input.pointer.filter(|_| input.hovered)?;
    Some(BlockDragEvent {
        position: pointer - input.rect.min,
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

pub(super) fn file_drop(input: &beui::ForwardedInput, dropping: bool) -> Option<FileDropEvent> {
    let mut hovering = dropping;
    let mut dropped = false;
    let mut files = Vec::new();
    for event in &input.events {
        match event {
            Event::FileHovered => hovering = true,
            Event::FileHoverCancelled => hovering = false,
            Event::FileDropped(file) => {
                dropped = true;
                files.extend(read_dropped(file.clone()));
            }
            _ => {}
        }
    }
    if !hovering && !dropped {
        return None;
    }
    let pointer = input.pointer.filter(|_| input.hovered)?;
    Some(FileDropEvent {
        position: pointer - input.rect.min,
        files,
        dropped,
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
    paste_shortcut_down: bool,
    captured_touches: HashSet<(u64, u64)>,
}

impl InputAdapter {
    pub(super) fn focused(&self) -> bool {
        self.focused
    }

    pub(super) fn forward(
        &mut self,
        input: &beui::ForwardedInput,
        screen: ScreenId,
    ) -> Vec<Message> {
        let rect = input.rect;
        let mut output = Vec::new();
        if input.focused != self.focused {
            output.push(InputEvent::Focus(input.focused));
            self.focused = input.focused;
            if !input.focused {
                self.captured = false;
                self.pressed_buttons = 0;
                self.captured_touches.clear();
            }
        }
        for event in &input.events {
            self.forward_event(event, rect, input.modifiers, &mut output);
        }
        if !input.hovered && self.pressed_buttons == 0 {
            self.captured = false;
            self.leave(&mut output);
        }
        let shortcut_down = super::clipboard::paste_shortcut_down();
        let shortcut_pressed = shortcut_down && !self.paste_shortcut_down;
        self.paste_shortcut_down = shortcut_down;
        if input.focused && shortcut_pressed {
            output.push(InputEvent::Paste(String::new()));
        }
        match output.is_empty() {
            true => Vec::new(),
            false => vec![Message::Input(InputBatch {
                screen,
                events: output,
            })],
        }
    }

    fn forward_event(
        &mut self,
        event: &Event,
        rect: Rect,
        modifiers: beui::Modifiers,
        output: &mut Vec<InputEvent>,
    ) {
        match event {
            Event::PointerMoved(position) => {
                self.pointer_inside = true;
                let position = *position - rect.min;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            Event::PointerGone => self.leave(output),
            Event::PointerMotion(delta) => output.push(InputEvent::PointerMotion {
                x: delta.x,
                y: delta.y,
            }),
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => {
                let button_mask = 1 << host::pointer_button_index(*button);
                self.pressed_buttons = match pressed {
                    true => self.pressed_buttons | button_mask,
                    false => self.pressed_buttons & !button_mask,
                };
                self.captured = self.pressed_buttons != 0;
                let position = *pos - rect.min;
                push_modifiers(&mut self.modifiers, *modifiers, output);
                output.push(InputEvent::PointerButton {
                    button: beui_plugin_input::pointer_button(*button),
                    pressed: *pressed,
                    x: position.x,
                    y: position.y,
                });
            }
            Event::Scroll(delta) => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: WheelUnit::Pixels,
                });
            }
            Event::ScrollEnded => output.push(InputEvent::WheelEnded),
            Event::Zoom(factor) => output.push(InputEvent::Zoom { factor: *factor }),
            Event::Touch {
                id,
                phase,
                pos,
                force,
            } => {
                let position = *pos - rect.min;
                output.push(InputEvent::Touch {
                    device: id.device,
                    finger: id.finger,
                    phase: beui_plugin_input::touch_phase(*phase),
                    x: position.x,
                    y: position.y,
                    force: *force,
                });
                if *phase == beui::TouchPhase::Cancel && self.pressed_buttons & 1 != 0 {
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
            } => {
                push_modifiers(&mut self.modifiers, *modifiers, output);
                output.push(InputEvent::Key {
                    key: beui_plugin_input::protocol_key(*key),
                    pressed: *pressed,
                    repeat: *repeat,
                });
            }
            Event::Modifiers(modifiers) => push_modifiers(&mut self.modifiers, *modifiers, output),
            Event::Text(text) => output.push(InputEvent::Text(text.clone())),
            Event::Ime(ime) => output.push(InputEvent::Ime(match ime {
                beui::ImeEvent::Enabled => ImeInput::Enabled,
                beui::ImeEvent::Preedit(text) => ImeInput::Preedit(text.clone()),
                beui::ImeEvent::Commit(text) => ImeInput::Commit(text.clone()),
                beui::ImeEvent::Disabled => ImeInput::Disabled,
            })),
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

    pub(super) fn back(&self, gesture: beui::BackGesture) -> InputEvent {
        InputEvent::Back(beui_plugin_input::back_phase(gesture))
    }

    fn leave(&mut self, output: &mut Vec<InputEvent>) {
        if self.pointer_inside && !self.captured {
            self.pointer_inside = false;
            output.push(InputEvent::PointerLeft);
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
    let modifiers = beui_plugin_input::protocol_modifiers(modifiers);
    if *previous != modifiers {
        *previous = modifiers;
        output.push(InputEvent::Modifiers(modifiers));
    }
}
