use block_plugin_api::{
    DroppedFile, ImeInput, InputBatch, InputEvent, Key, Message, Modifiers, PointerButton,
    ScreenId, ViewportMetrics, WheelUnit,
};
use eframe::egui;
use std::collections::HashSet;
use uuid::Uuid;

use super::instances::Holes;
use crate::editors::SidebarDragPayload;

pub(super) struct BlockDragEvent {
    pub(super) position: egui::Vec2,
    pub(super) block_id: Uuid,
    pub(super) block_type: Uuid,
    pub(super) dropped: bool,
}

pub(super) fn block_drag(response: &egui::Response) -> Option<BlockDragEvent> {
    let (payload, dropped) = match response.dnd_release_payload::<SidebarDragPayload>() {
        Some(payload) => (payload, true),
        None => (response.dnd_hover_payload::<SidebarDragPayload>()?, false),
    };
    let pointer = response
        .ctx
        .pointer_interact_pos()
        .unwrap_or_else(|| response.rect.center());
    Some(BlockDragEvent {
        position: pointer - response.rect.min,
        block_id: payload.block_id,
        block_type: payload.block_type,
        dropped,
    })
}

pub(super) struct FileDropEvent {
    pub(super) position: egui::Vec2,
    pub(super) files: Vec<DroppedFile>,
    pub(super) dropped: bool,
}

pub(super) fn file_drop(response: &egui::Response) -> Option<FileDropEvent> {
    let (hovering, dropped) = response.ctx.input(|input| {
        (
            !input.raw.hovered_files.is_empty(),
            input.raw.dropped_files.clone(),
        )
    });
    let pointer = response
        .ctx
        .pointer_latest_pos()
        .filter(|position| response.rect.contains(*position))?;
    if !dropped.is_empty() {
        return Some(FileDropEvent {
            position: pointer - response.rect.min,
            files: dropped.into_iter().filter_map(read_dropped).collect(),
            dropped: true,
        });
    }
    hovering.then(|| FileDropEvent {
        position: pointer - response.rect.min,
        files: Vec::new(),
        dropped: false,
    })
}

fn read_dropped(file: egui::DroppedFile) -> Option<DroppedFile> {
    let name = file
        .path
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| (!file.name.is_empty()).then(|| file.name.clone()))
        .unwrap_or_else(|| "File".to_owned());
    let data = match file.bytes {
        Some(bytes) => bytes.to_vec(),
        None => std::fs::read(file.path.as_ref()?).ok()?,
    };
    Some(DroppedFile { name, data })
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
        context: &egui::Context,
        rect: egui::Rect,
        hovered: bool,
        focused: bool,
        screen: ScreenId,
        holes: &Holes,
    ) -> Vec<Message> {
        self.over_hole = !self.captured
            && context
                .pointer_latest_pos()
                .is_some_and(|position| holes.contains(position));
        let events = context.input(|input| input.events.clone());
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
                self.normalize_event(event, rect, hovered, focused, holes, &mut normalized);
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

    fn normalize_event(
        &mut self,
        event: egui::Event,
        rect: egui::Rect,
        hovered: bool,
        focused: bool,
        holes: &Holes,
        output: &mut Vec<InputEvent>,
    ) {
        let pointer = |position: egui::Pos2, captured: bool| {
            (rect.contains(position) && !holes.contains(position)) || captured
        };
        match event {
            egui::Event::PointerMoved(position) if pointer(position, self.captured) => {
                self.pointer_inside = true;
                let position = position - rect.min;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            egui::Event::PointerMoved(_) | egui::Event::PointerGone => self.leave(output),
            egui::Event::MouseMoved(delta) if focused => {
                output.push(InputEvent::PointerMotion {
                    x: delta.x,
                    y: delta.y,
                });
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } if pointer(pos, self.captured) => {
                let button_mask = 1 << pointer_button_index(button);
                self.pressed_buttons = if pressed {
                    self.pressed_buttons | button_mask
                } else {
                    self.pressed_buttons & !button_mask
                };
                self.captured = self.pressed_buttons != 0;
                let position = pos - rect.min;
                #[cfg(target_os = "windows")]
                eprintln!(
                    "plugin input host pointer button={button:?} pressed={pressed} window=({:.1},{:.1}) local=({:.1},{:.1}) viewport=({:.1},{:.1})",
                    pos.x,
                    pos.y,
                    position.x,
                    position.y,
                    rect.width(),
                    rect.height()
                );
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::PointerButton {
                    button: pointer_button(button),
                    pressed,
                    x: position.x,
                    y: position.y,
                });
                if !pointer(pos, false) {
                    self.leave(output);
                }
            }
            egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } if hovered && !self.over_hole => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: wheel_unit(unit),
                });
            }
            egui::Event::Zoom(factor) if hovered && !self.over_hole => {
                output.push(InputEvent::Zoom { factor });
            }
            egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                force,
            } => {
                let touch = (device_id.0, id.0);
                let accepted = match phase {
                    egui::TouchPhase::Start => pointer(pos, false),
                    egui::TouchPhase::Move | egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.captured_touches.contains(&touch)
                    }
                };
                if !accepted {
                    return;
                }
                if phase == egui::TouchPhase::Start {
                    self.captured_touches.insert(touch);
                }
                let position = pos - rect.min;
                output.push(InputEvent::Touch {
                    device: device_id.0,
                    finger: id.0,
                    phase: touch_phase(phase),
                    x: position.x,
                    y: position.y,
                    force,
                });
                if matches!(phase, egui::TouchPhase::End | egui::TouchPhase::Cancel) {
                    self.captured_touches.remove(&touch);
                }
                if phase == egui::TouchPhase::Cancel && self.pressed_buttons & 1 != 0 {
                    self.pressed_buttons &=
                        !(1 << pointer_button_index(egui::PointerButton::Primary));
                    self.captured = self.pressed_buttons != 0;
                    output.push(InputEvent::PointerButton {
                        button: PointerButton::Primary,
                        pressed: false,
                        x: position.x,
                        y: position.y,
                    });
                }
            }
            egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } if focused => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Key {
                    key: protocol_key(key),
                    pressed,
                    repeat,
                });
            }
            egui::Event::Text(text) if focused => {
                output.push(InputEvent::Text(text));
            }
            egui::Event::Paste(text) if focused => {
                output.push(InputEvent::Paste(text));
            }
            egui::Event::Ime(ime) if focused => {
                output.push(InputEvent::Ime(match ime {
                    egui::ImeEvent::Enabled => ImeInput::Enabled,
                    egui::ImeEvent::Preedit(text) => ImeInput::Preedit(text),
                    egui::ImeEvent::Commit(text) => ImeInput::Commit(text),
                    egui::ImeEvent::Disabled => ImeInput::Disabled,
                }));
            }
            egui::Event::WindowFocused(window_focused) if !window_focused && self.focused => {
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

pub(super) fn viewport_metrics(
    size: egui::Vec2,
    visible: egui::Rect,
    scale_factor: f32,
) -> ViewportMetrics {
    let logical_width = size.x.max(0.0);
    let logical_height = size.y.max(0.0);
    let visible = visible.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
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
    modifiers: egui::Modifiers,
    output: &mut Vec<InputEvent>,
) {
    let modifiers = Modifiers {
        alt: modifiers.alt,
        control: modifiers.ctrl,
        shift: modifiers.shift,
        command: modifiers.command,
    };
    if *previous != modifiers {
        *previous = modifiers;
        output.push(InputEvent::Modifiers(modifiers));
    }
}

fn pointer_button(button: egui::PointerButton) -> PointerButton {
    match button {
        egui::PointerButton::Primary => PointerButton::Primary,
        egui::PointerButton::Secondary => PointerButton::Secondary,
        egui::PointerButton::Middle => PointerButton::Middle,
        egui::PointerButton::Extra1 => PointerButton::Back,
        egui::PointerButton::Extra2 => PointerButton::Forward,
    }
}

fn pointer_button_index(button: egui::PointerButton) -> u8 {
    match button {
        egui::PointerButton::Primary => 0,
        egui::PointerButton::Secondary => 1,
        egui::PointerButton::Middle => 2,
        egui::PointerButton::Extra1 => 3,
        egui::PointerButton::Extra2 => 4,
    }
}

fn wheel_unit(unit: egui::MouseWheelUnit) -> WheelUnit {
    match unit {
        egui::MouseWheelUnit::Point => WheelUnit::Pixels,
        egui::MouseWheelUnit::Line => WheelUnit::Lines,
        egui::MouseWheelUnit::Page => WheelUnit::Pages,
    }
}

fn touch_phase(phase: egui::TouchPhase) -> block_plugin_api::TouchPhase {
    match phase {
        egui::TouchPhase::Start => block_plugin_api::TouchPhase::Start,
        egui::TouchPhase::Move => block_plugin_api::TouchPhase::Move,
        egui::TouchPhase::End => block_plugin_api::TouchPhase::End,
        egui::TouchPhase::Cancel => block_plugin_api::TouchPhase::Cancel,
    }
}

pub(super) fn protocol_key(key: egui::Key) -> Key {
    match key {
        egui::Key::ArrowDown => Key::ArrowDown,
        egui::Key::ArrowLeft => Key::ArrowLeft,
        egui::Key::ArrowRight => Key::ArrowRight,
        egui::Key::ArrowUp => Key::ArrowUp,
        egui::Key::Escape => Key::Escape,
        egui::Key::Tab => Key::Tab,
        egui::Key::Backspace => Key::Backspace,
        egui::Key::Enter => Key::Enter,
        egui::Key::Space => Key::Space,
        egui::Key::Insert => Key::Insert,
        egui::Key::Delete => Key::Delete,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::PageUp => Key::PageUp,
        egui::Key::PageDown => Key::PageDown,
        egui::Key::Copy => Key::Copy,
        egui::Key::Cut => Key::Cut,
        egui::Key::Paste => Key::Paste,
        egui::Key::Colon => Key::Colon,
        egui::Key::Comma => Key::Comma,
        egui::Key::Backslash => Key::Backslash,
        egui::Key::Slash => Key::Slash,
        egui::Key::Pipe => Key::Pipe,
        egui::Key::Questionmark => Key::Questionmark,
        egui::Key::Exclamationmark => Key::Exclamationmark,
        egui::Key::OpenBracket => Key::OpenBracket,
        egui::Key::CloseBracket => Key::CloseBracket,
        egui::Key::OpenCurlyBracket => Key::OpenCurlyBracket,
        egui::Key::CloseCurlyBracket => Key::CloseCurlyBracket,
        egui::Key::Backtick => Key::Backtick,
        egui::Key::Minus => Key::Minus,
        egui::Key::Period => Key::Period,
        egui::Key::Plus => Key::Plus,
        egui::Key::Equals => Key::Equals,
        egui::Key::Semicolon => Key::Semicolon,
        egui::Key::Quote => Key::Quote,
        egui::Key::Num0 => Key::Num0,
        egui::Key::Num1 => Key::Num1,
        egui::Key::Num2 => Key::Num2,
        egui::Key::Num3 => Key::Num3,
        egui::Key::Num4 => Key::Num4,
        egui::Key::Num5 => Key::Num5,
        egui::Key::Num6 => Key::Num6,
        egui::Key::Num7 => Key::Num7,
        egui::Key::Num8 => Key::Num8,
        egui::Key::Num9 => Key::Num9,
        egui::Key::A => Key::A,
        egui::Key::B => Key::B,
        egui::Key::C => Key::C,
        egui::Key::D => Key::D,
        egui::Key::E => Key::E,
        egui::Key::F => Key::F,
        egui::Key::G => Key::G,
        egui::Key::H => Key::H,
        egui::Key::I => Key::I,
        egui::Key::J => Key::J,
        egui::Key::K => Key::K,
        egui::Key::L => Key::L,
        egui::Key::M => Key::M,
        egui::Key::N => Key::N,
        egui::Key::O => Key::O,
        egui::Key::P => Key::P,
        egui::Key::Q => Key::Q,
        egui::Key::R => Key::R,
        egui::Key::S => Key::S,
        egui::Key::T => Key::T,
        egui::Key::U => Key::U,
        egui::Key::V => Key::V,
        egui::Key::W => Key::W,
        egui::Key::X => Key::X,
        egui::Key::Y => Key::Y,
        egui::Key::Z => Key::Z,
        egui::Key::F1 => Key::F1,
        egui::Key::F2 => Key::F2,
        egui::Key::F3 => Key::F3,
        egui::Key::F4 => Key::F4,
        egui::Key::F5 => Key::F5,
        egui::Key::F6 => Key::F6,
        egui::Key::F7 => Key::F7,
        egui::Key::F8 => Key::F8,
        egui::Key::F9 => Key::F9,
        egui::Key::F10 => Key::F10,
        egui::Key::F11 => Key::F11,
        egui::Key::F12 => Key::F12,
        egui::Key::F13 => Key::F13,
        egui::Key::F14 => Key::F14,
        egui::Key::F15 => Key::F15,
        egui::Key::F16 => Key::F16,
        egui::Key::F17 => Key::F17,
        egui::Key::F18 => Key::F18,
        egui::Key::F19 => Key::F19,
        egui::Key::F20 => Key::F20,
        egui::Key::F21 => Key::F21,
        egui::Key::F22 => Key::F22,
        egui::Key::F23 => Key::F23,
        egui::Key::F24 => Key::F24,
        egui::Key::F25 => Key::F25,
        egui::Key::F26 => Key::F26,
        egui::Key::F27 => Key::F27,
        egui::Key::F28 => Key::F28,
        egui::Key::F29 => Key::F29,
        egui::Key::F30 => Key::F30,
        egui::Key::F31 => Key::F31,
        egui::Key::F32 => Key::F32,
        egui::Key::F33 => Key::F33,
        egui::Key::F34 => Key::F34,
        egui::Key::F35 => Key::F35,
        egui::Key::BrowserBack => Key::BrowserBack,
    }
}

#[cfg(test)]
mod tests;
