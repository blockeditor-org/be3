use beui::{Event, Forwarded, PointerEvent, PointerId, PointerPhase, Pos2, Rect, Vec2};
use block_plugin_api::{
    DroppedFile, InputBatch, InputEvent, Message, Modifiers, Monitor, PenPhase, ScreenId,
    TouchPhase, ViewportMetrics, WheelUnit,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockDrag {
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
}

pub(super) struct BlockDragEvent {
    pub(super) position: Vec2,
    pub(super) drag: BlockDrag,
    pub(super) dropped: bool,
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
    for event in input.inputs() {
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

fn read_path(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[derive(Default)]
pub(super) struct InputAdapter {
    focused: bool,
    modifiers: Modifiers,
}

impl InputAdapter {
    pub(super) fn focused(&self) -> bool {
        self.focused
    }

    pub(super) fn forward(
        &mut self,
        input: &beui::ForwardedInput,
        screen: ScreenId,
        zoom: f32,
    ) -> Vec<Message> {
        let zoom = match zoom.is_finite() && zoom > 0.0 {
            true => zoom,
            false => 1.0,
        };
        let mut output = Vec::new();
        for event in &input.events {
            self.forward_event(event, input.rect, zoom, &mut output);
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
        event: &Forwarded,
        rect: Rect,
        zoom: f32,
        output: &mut Vec<InputEvent>,
    ) {
        match event {
            Forwarded::Pointer { event, .. } => self.pointer(event, rect, zoom, output),
            Forwarded::Left => output.push(InputEvent::PointerLeft),
            Forwarded::Focus(focused) | Forwarded::Input(Event::Focus(focused)) => {
                if self.focused != *focused {
                    self.focused = *focused;
                    output.push(InputEvent::Focus(*focused));
                }
            }
            Forwarded::Input(event) => self.input(event, output),
        }
    }

    fn pointer(
        &mut self,
        event: &PointerEvent,
        rect: Rect,
        zoom: f32,
        output: &mut Vec<InputEvent>,
    ) {
        let position = (event.pos - rect.min) / zoom;
        let (x, y) = (position.x, position.y);
        if let PointerId::Touch(id) = event.pointer {
            let phase = match event.phase {
                PointerPhase::Down(_) => TouchPhase::Start,
                PointerPhase::Move => TouchPhase::Move,
                PointerPhase::Up(_) => TouchPhase::End,
                PointerPhase::Cancel => TouchPhase::Cancel,
                _ => return,
            };
            output.push(InputEvent::Touch {
                device: id.device,
                finger: id.finger,
                phase,
                x,
                y,
                force: event.force,
            });
            return;
        }
        if event.pointer == PointerId::Pen {
            let phase = match event.phase {
                PointerPhase::Move => PenPhase::Move,
                PointerPhase::Down(button) => PenPhase::Down(button),
                PointerPhase::Up(button) => PenPhase::Up(button),
                PointerPhase::Cancel | PointerPhase::Gone => PenPhase::Gone,
                _ => return,
            };
            if matches!(phase, PenPhase::Down(_) | PenPhase::Up(_)) {
                push_modifiers(&mut self.modifiers, event.modifiers, output);
            }
            let pen = event.pen.unwrap_or_default();
            output.push(InputEvent::Pen {
                phase,
                x,
                y,
                pressure: event.force.unwrap_or(0.0),
                tilt_x: pen.tilt.x,
                tilt_y: pen.tilt.y,
                eraser: pen.eraser,
            });
            return;
        }
        match event.phase {
            PointerPhase::Move => output.push(InputEvent::PointerMoved { x, y }),
            PointerPhase::Down(button) | PointerPhase::Up(button) => {
                push_modifiers(&mut self.modifiers, event.modifiers, output);
                output.push(InputEvent::PointerButton {
                    button,
                    pressed: matches!(event.phase, PointerPhase::Down(_)),
                    x,
                    y,
                });
            }
            PointerPhase::Scroll(delta) => {
                push_modifiers(&mut self.modifiers, event.modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x / zoom,
                    y: delta.y / zoom,
                    unit: WheelUnit::Pixels,
                });
            }
            PointerPhase::ScrollEnded { .. } => output.push(InputEvent::WheelEnded),
            PointerPhase::Zoom(factor) => output.push(InputEvent::Zoom { factor }),
            PointerPhase::Cancel | PointerPhase::Gone => {}
        }
    }

    fn input(&mut self, event: &Event, output: &mut Vec<InputEvent>) {
        match event {
            Event::PointerMotion(delta) => output.push(InputEvent::PointerMotion {
                x: delta.x,
                y: delta.y,
            }),
            Event::Key { press, text, .. } => {
                push_modifiers(&mut self.modifiers, press.modifiers, output);
                match press.key {
                    beui::Key::Unidentified => output.extend(text.clone().map(InputEvent::Text)),
                    key => output.push(InputEvent::Key {
                        key,
                        pressed: press.pressed,
                        repeat: press.repeat,
                        text: text.clone(),
                    }),
                }
            }
            Event::Modifiers(modifiers) => push_modifiers(&mut self.modifiers, *modifiers, output),
            Event::Text(text) => output.push(InputEvent::Text(text.clone())),
            Event::Paste(beui::Pasted::Text(text)) => {
                output.extend(block_plugin_api::paste_events(text))
            }
            Event::Paste(pasted) => output.push(InputEvent::Paste(pasted.clone())),
            Event::Ime(ime) => output.push(InputEvent::Ime(ime.clone())),
            _ => {}
        }
    }

    pub(super) fn intercepted(
        &mut self,
        press: Option<beui::KeyPress>,
        modifiers: beui::Modifiers,
    ) -> Vec<InputEvent> {
        let mut output = Vec::new();
        push_modifiers(&mut self.modifiers, modifiers, &mut output);
        if let Some(press) = press
            && press.key != beui::Key::Unidentified
        {
            output.push(InputEvent::InterceptedKey {
                key: press.key,
                pressed: press.pressed,
                repeat: press.repeat,
            });
        }
        output
    }

    pub(super) fn back(&self, gesture: beui::BackGesture) -> InputEvent {
        InputEvent::Back(gesture)
    }
}

pub(super) fn viewport_metrics(
    size: Vec2,
    visible: Rect,
    scale_factor: f32,
    monitors: Vec<Monitor>,
) -> ViewportMetrics {
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
        monitors,
    }
}

fn push_modifiers(
    previous: &mut Modifiers,
    modifiers: beui::Modifiers,
    output: &mut Vec<InputEvent>,
) {
    if *previous != modifiers {
        *previous = modifiers;
        output.push(InputEvent::Modifiers(modifiers));
    }
}
