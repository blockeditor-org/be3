use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use block_editor_plugin::wgpu;
use block_editor_plugin::{
    Artifact, ArtifactDescription, EditorHost, EditorRegion, Frame, Instance, PaintTarget, Region,
};
use block_plugin_api::{CursorIcon, InputEvent, Key, PointerButton, WheelUnit};
use uuid::Uuid;

use crate::beui_frame::{self, BeuiFrame, FrameBar};
use crate::editor::BeuiScale;
use crate::{Artifacts, BeuiApp, Creation, Editor};

const WHEEL_LINE: f32 = 40.0;
const WHEEL_PAGE: f32 = 400.0;

pub struct BeuiPlugin<A: BeuiApp>(PhantomData<A>);

impl<A: BeuiApp> block_editor_plugin::Plugin for BeuiPlugin<A> {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(BeuiInstance::<A>::new(host))
    }
}

struct BeuiInstance<A: BeuiApp> {
    host: EditorHost,
    scale: Rc<Cell<BeuiScale>>,
    regions: HashMap<EditorRegion, BeuiRegion>,
    views: Views<A>,
    renderer: Option<beui::Renderer>,
}

struct BeuiRegion {
    context: beui::Context,
    events: Vec<beui::Event>,
    modifiers: beui::Modifiers,
    pointer: beui::Pos2,
    emulated_touch: bool,
    chrome: Option<BeuiFrame>,
    output: Option<beui::FrameOutput>,
}

impl BeuiRegion {
    fn new() -> Self {
        Self {
            context: beui::Context::new(),
            events: Vec::new(),
            modifiers: beui::Modifiers::NONE,
            pointer: beui::Pos2::ZERO,
            emulated_touch: false,
            chrome: None,
            output: None,
        }
    }

    fn emulate_touch(&mut self, phase: beui::TouchPhase) {
        self.events.push(beui::Event::Touch {
            id: beui::TouchId {
                device: 0,
                finger: 0,
            },
            phase,
            pos: self.pointer,
            force: None,
        });
    }
}

struct Views<A: BeuiApp> {
    creating: bool,
    editor: Option<Editor>,
    preview: Option<Editor>,
    preview_document: Option<beui::Document>,
    creation: Option<Creation>,
    dialog: Option<beui::Document>,
    artifacts: Option<Artifacts>,
    settings: Option<beui::Document>,
    app: PhantomData<A>,
}

impl<A: BeuiApp> Views<A> {
    fn preview(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(editor) = self.preview.clone() else {
            return;
        };
        let document = self.preview_document.get_or_insert_with(|| {
            let built = editor.clone();
            beui::reactive::build(move || A::preview_view(built))
        });
        let begun = editor.clone();
        beui::reactive::with_reactive_scope(document, move || begun.begin_frame());
        document.show(context, rect);
        editor.end_frame(document);
    }

    fn creation(&mut self, context: &beui::Context, rect: beui::Rect) {
        let (Some(creation), Some(dialog)) = (self.creation.as_ref(), self.dialog.as_mut()) else {
            return;
        };
        let creation = creation.clone();
        beui::reactive::with_reactive_scope(dialog, move || creation.begin_frame());
        dialog.show(context, rect);
    }

    fn artifact_settings(
        &mut self,
        context: &beui::Context,
        rect: beui::Rect,
        draft: &mut Vec<u8>,
    ) {
        let Some(artifacts) = self.artifacts.clone() else {
            return;
        };
        let document = self.settings.get_or_insert_with(|| {
            let built = artifacts.clone();
            beui::reactive::build(move || A::artifact_settings_view(built))
        });
        let received = artifacts.clone();
        let data = std::mem::take(draft);
        beui::reactive::with_reactive_scope(document, move || received.receive_settings(&data));
        document.show(context, rect);
        *draft = artifacts
            .take_settings_edit()
            .unwrap_or_else(|| artifacts.settings().get_untracked());
    }
}

impl<A: BeuiApp> BeuiInstance<A> {
    fn new(host: EditorHost) -> Self {
        Self {
            host,
            scale: Rc::default(),
            regions: HashMap::new(),
            views: Views {
                creating: false,
                editor: None,
                preview: None,
                preview_document: None,
                creation: None,
                dialog: None,
                artifacts: None,
                settings: None,
                app: PhantomData,
            },
            renderer: None,
        }
    }
}

impl<A: BeuiApp> Instance for BeuiInstance<A> {
    fn connect(&mut self, block_id: Uuid) {
        let scaled = |host: EditorHost| Editor::scaled(host, block_id, Rc::clone(&self.scale));
        self.views.editor = Some(scaled(self.host.clone()));
        self.views.preview = Some(scaled(self.host.clone()));
        self.views.preview_document = None;
    }

    fn connect_creation(&mut self) {
        self.views.creating = true;
        let creation = Creation::new(self.host.clone());
        let built = creation.clone();
        self.views.dialog = Some(beui::reactive::build(move || A::creation_view(built)));
        self.views.creation = Some(creation);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .views
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        A::create_block(creation)
    }

    fn connect_artifact(&mut self, artifact: Artifact) {
        let artifacts = Artifacts::new(self.host.clone(), artifact);
        A::connect_artifact(&artifacts);
        self.views.artifacts = Some(artifacts);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> Result<ArtifactDescription, String> {
        A::describe_artifact(data)
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        if let Some(artifacts) = &self.views.artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.views.artifacts.as_ref()?.poll()
    }

    fn intrinsic_size(&mut self) -> Option<beui::Vec2> {
        self.views
            .editor
            .as_ref()
            .and_then(Editor::intrinsic_size)
            .or_else(A::intrinsic_size)
    }

    fn resized(&mut self, size: beui::Vec2) {
        if let Some(editor) = &self.views.editor {
            editor.report_resize(size);
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, visible: bool) {
        if let Some(editor) = &self.views.editor {
            editor.report_presence_visible(visible);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(editor) = self.views.editor.clone() else {
            return false;
        };
        let document = self
            .regions
            .get_mut(&EditorRegion::Frame)
            .and_then(|region| region.chrome.as_mut())
            .map(BeuiFrame::document_mut);
        match document {
            Some(document) => {
                beui::reactive::with_reactive_scope(document, || editor.replace_child(old, new))
            }
            None => editor.replace_child(old, new),
        }
    }

    fn update(&mut self, region: &Region, settings: Option<&mut Vec<u8>>) -> Frame {
        let state = self
            .regions
            .entry(region.region)
            .or_insert_with(BeuiRegion::new);
        state.context.set_pixels_per_point(region.scale_factor);
        let pixels_per_point = state.context.pixels_per_point();
        let ratio = region.scale_factor / pixels_per_point;
        self.scale.set(BeuiScale {
            ratio,
            pixels_per_point,
        });
        let events = std::mem::take(&mut state.events);
        let context = state.context.clone();
        let frame = region.rect.scaled(ratio);
        let spec = &region.spec;
        let drawn = region.chrome_drawn();
        let views = &mut self.views;
        if region.region == EditorRegion::Frame && !views.creating && state.chrome.is_none() {
            let editor = views
                .editor
                .clone()
                .expect("connect is called before the view is built");
            let built = editor.clone();
            state.chrome = Some(BeuiFrame::build(&editor, move || A::view(built)));
        }
        let mut exit = false;
        let mut content = None;
        let mut painted = Vec::new();
        let mut floating = Vec::new();
        let output = context.run(beui::RawInput { events }, |context| match region.region {
            EditorRegion::Frame if views.creating => views.creation(context, frame),
            EditorRegion::Frame => {
                let chrome = state
                    .chrome
                    .as_mut()
                    .expect("the frame chrome was just built");
                let set_bar = chrome.set_bar();
                let bar = FrameBar {
                    shown: drawn && (spec.top_bar || spec.content.is_some()),
                    closable: spec.content.is_some(),
                };
                let editor = views.editor.clone();
                beui::reactive::with_reactive_scope(chrome.document_mut(), || {
                    set_bar.set(bar);
                    if let Some(editor) = &editor {
                        editor.begin_frame();
                    }
                });
                chrome.document_mut().show(context, frame);
                if let Some(editor) = &editor {
                    editor.end_frame(chrome.document());
                }
                content = chrome.document().node_rect(chrome.content());
                exit = chrome.exit().get() || beui_frame::escaped(context);
                painted = vec![frame];
                floating = chrome.document().overlay_rects();
            }
            EditorRegion::Preview => views.preview(context, frame),
            EditorRegion::ArtifactSettings => {
                if let Some(draft) = settings {
                    views.artifact_settings(context, frame, draft);
                }
            }
        });
        if exit {
            self.host.leave_frame();
        }
        if let Some(text) = &output.copied_text {
            self.host.copy_text(text.clone());
        }
        if output.paste_requested {
            self.host.request_paste();
        }
        let unscale = ratio.recip();
        let result = Frame {
            changed: output.changed,
            repaint_after: (output.repaint_after < Duration::MAX).then_some(output.repaint_after),
            cursor: match context.touch_emulation() {
                true => CursorIcon::Crosshair,
                false => beui_cursor(output.cursor_icon),
            },
            content: content.map(|rect| rect.scaled(unscale)),
            painted: painted.iter().map(|rect| rect.scaled(unscale)).collect(),
            floating: floating.iter().map(|rect| rect.scaled(unscale)).collect(),
        };
        state.output = Some(output);
        let locked = self
            .regions
            .values()
            .any(|region| region.context.pointer_locked());
        self.host.grab_cursor(locked);
        result
    }

    fn paint(&mut self, target: &PaintTarget<'_>) {
        let Some(output) = self
            .regions
            .get(&target.placement.region)
            .and_then(|region| region.output.as_ref())
        else {
            return;
        };
        let renderer = self
            .renderer
            .get_or_insert_with(|| beui::Renderer::new(target.device, target.format));
        let screen = beui::vec2(target.width as f32, target.height as f32);
        let _ = renderer.prepare(
            target.device,
            target.queue,
            output,
            screen,
            output.pixels_per_point(),
            beui::Repaint::Everything,
        );
        let mut encoder = target
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plugin beui pane"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let (x, y, width, height) = target.scissor();
            pass.set_scissor_rect(x, y, width, height);
            renderer.paint(&mut pass);
        }
        target.queue.submit([encoder.finish()]);
    }

    fn input(&mut self, region: &Region, event: &InputEvent) {
        let scale_factor = region.scale_factor;
        let origin = region.rect.min.to_vec2();
        let state = self
            .regions
            .entry(region.region)
            .or_insert_with(BeuiRegion::new);
        let ratio = state
            .context
            .simulated_pixels_per_point()
            .map_or(1.0, |simulated| scale_factor / simulated);
        let at = |x: f32, y: f32| beui::pos2((x + origin.x) * ratio, (y + origin.y) * ratio);
        let emulating = state.context.touch_emulation();
        if !emulating && state.emulated_touch {
            state.emulated_touch = false;
            state.emulate_touch(beui::TouchPhase::Cancel);
        }
        match event {
            InputEvent::PointerMoved { x, y } => {
                state.pointer = at(*x, *y);
                if !emulating {
                    state.events.push(beui::Event::PointerMoved(state.pointer));
                } else if state.emulated_touch {
                    state.emulate_touch(beui::TouchPhase::Move);
                }
            }
            InputEvent::PointerLeft if !emulating => state.events.push(beui::Event::PointerGone),
            InputEvent::PointerLeft => {}
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed,
                x,
                y,
            } if emulating => {
                state.pointer = at(*x, *y);
                if *pressed != state.emulated_touch {
                    state.emulated_touch = *pressed;
                    state.emulate_touch(if *pressed {
                        beui::TouchPhase::Start
                    } else {
                        beui::TouchPhase::End
                    });
                }
            }
            InputEvent::PointerButton { .. } if emulating => {}
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                let Some(button) = beui_button(*button) else {
                    return;
                };
                state.pointer = at(*x, *y);
                state.events.push(beui::Event::PointerButton {
                    pos: state.pointer,
                    button,
                    pressed: *pressed,
                    modifiers: state.modifiers,
                });
            }
            InputEvent::Wheel { x, y, unit } => {
                let scale = match unit {
                    WheelUnit::Pixels => 1.0,
                    WheelUnit::Lines => WHEEL_LINE,
                    WheelUnit::Pages => WHEEL_PAGE,
                };
                state
                    .events
                    .push(beui::Event::Scroll(beui::vec2(x * scale, y * scale)));
            }
            InputEvent::Touch {
                device,
                finger,
                phase,
                x,
                y,
                force,
            } => state.events.push(beui::Event::Touch {
                id: beui::TouchId {
                    device: *device,
                    finger: *finger,
                },
                phase: beui_touch_phase(*phase),
                pos: at(*x, *y),
                force: *force,
            }),
            InputEvent::Key {
                key,
                pressed,
                repeat,
            } => {
                let Some(key) = beui_key(*key) else {
                    return;
                };
                state.events.push(beui::Event::Key {
                    key,
                    pressed: *pressed,
                    repeat: *repeat,
                    modifiers: state.modifiers,
                });
            }
            InputEvent::Text(text) | InputEvent::Paste(text) => {
                state.events.push(beui::Event::Text(text.clone()));
            }
            InputEvent::Modifiers(modifiers) => {
                state.modifiers = beui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control || modifiers.command,
                    shift: modifiers.shift,
                };
                state.events.push(beui::Event::Modifiers(state.modifiers));
            }
            InputEvent::Zoom { factor } => {
                state.events.push(beui::Event::Zoom(*factor));
            }
            InputEvent::PointerMotion { x, y } => {
                state
                    .events
                    .push(beui::Event::PointerMotion(beui::vec2(*x, *y) * ratio));
            }
            InputEvent::Focus(false) => {
                state.emulated_touch = false;
                state.events.push(beui::Event::Focus(false));
            }
            InputEvent::Ime(_) | InputEvent::Focus(_) => {}
        }
    }
}

fn beui_button(button: PointerButton) -> Option<beui::PointerButton> {
    match button {
        PointerButton::Primary => Some(beui::PointerButton::Primary),
        PointerButton::Secondary => Some(beui::PointerButton::Secondary),
        PointerButton::Middle => Some(beui::PointerButton::Middle),
        PointerButton::Back | PointerButton::Forward | PointerButton::Other(_) => None,
    }
}

fn beui_touch_phase(phase: block_plugin_api::TouchPhase) -> beui::TouchPhase {
    match phase {
        block_plugin_api::TouchPhase::Start => beui::TouchPhase::Start,
        block_plugin_api::TouchPhase::Move => beui::TouchPhase::Move,
        block_plugin_api::TouchPhase::End => beui::TouchPhase::End,
        block_plugin_api::TouchPhase::Cancel => beui::TouchPhase::Cancel,
    }
}

fn beui_key(key: Key) -> Option<beui::Key> {
    let key = match key {
        Key::ArrowDown => beui::Key::ArrowDown,
        Key::ArrowLeft => beui::Key::ArrowLeft,
        Key::ArrowRight => beui::Key::ArrowRight,
        Key::ArrowUp => beui::Key::ArrowUp,
        Key::Backspace => beui::Key::Backspace,
        Key::Delete => beui::Key::Delete,
        Key::End => beui::Key::End,
        Key::Enter => beui::Key::Enter,
        Key::Escape => beui::Key::Escape,
        Key::Home => beui::Key::Home,
        Key::OpenBracket => beui::Key::BracketLeft,
        Key::CloseBracket => beui::Key::BracketRight,
        Key::Minus => beui::Key::Minus,
        Key::PageDown => beui::Key::PageDown,
        Key::PageUp => beui::Key::PageUp,
        Key::Plus | Key::Equals => beui::Key::Plus,
        Key::Space => beui::Key::Space,
        Key::Tab => beui::Key::Tab,
        Key::Num0 => beui::Key::Zero,
        Key::Num1 => beui::Key::One,
        Key::Num2 => beui::Key::Two,
        Key::Num3 => beui::Key::Three,
        Key::Num4 => beui::Key::Four,
        Key::Num5 => beui::Key::Five,
        Key::Num6 => beui::Key::Six,
        Key::Num7 => beui::Key::Seven,
        Key::Num8 => beui::Key::Eight,
        Key::Num9 => beui::Key::Nine,
        Key::Backtick => beui::Key::Backtick,
        Key::A => beui::Key::A,
        Key::B => beui::Key::B,
        Key::C => beui::Key::C,
        Key::D => beui::Key::D,
        Key::E => beui::Key::E,
        Key::F => beui::Key::F,
        Key::G => beui::Key::G,
        Key::H => beui::Key::H,
        Key::I => beui::Key::I,
        Key::J => beui::Key::J,
        Key::K => beui::Key::K,
        Key::L => beui::Key::L,
        Key::M => beui::Key::M,
        Key::N => beui::Key::N,
        Key::O => beui::Key::O,
        Key::P => beui::Key::P,
        Key::Q => beui::Key::Q,
        Key::R => beui::Key::R,
        Key::S => beui::Key::S,
        Key::T => beui::Key::T,
        Key::U => beui::Key::U,
        Key::V => beui::Key::V,
        Key::W => beui::Key::W,
        Key::X => beui::Key::X,
        Key::Y => beui::Key::Y,
        Key::Z => beui::Key::Z,
        _ => return None,
    };
    Some(key)
}

fn beui_cursor(cursor: beui::CursorIcon) -> CursorIcon {
    match cursor {
        beui::CursorIcon::Default => CursorIcon::Default,
        beui::CursorIcon::Crosshair => CursorIcon::Crosshair,
        beui::CursorIcon::Grab => CursorIcon::Grab,
        beui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        beui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
        beui::CursorIcon::PointingHand => CursorIcon::Pointer,
        beui::CursorIcon::ResizeHorizontal => CursorIcon::ResizeHorizontal,
        beui::CursorIcon::ResizeVertical => CursorIcon::ResizeVertical,
        beui::CursorIcon::ResizeNeSw => CursorIcon::ResizeNeSw,
        beui::CursorIcon::ResizeNwSe => CursorIcon::ResizeNwSe,
        beui::CursorIcon::Text => CursorIcon::Text,
        beui::CursorIcon::Wait => CursorIcon::Wait,
        beui::CursorIcon::None => CursorIcon::None,
        beui::CursorIcon::Move => CursorIcon::Move,
        beui::CursorIcon::Progress => CursorIcon::Progress,
        beui::CursorIcon::Help => CursorIcon::Help,
        beui::CursorIcon::Alias => CursorIcon::Pointer,
    }
}

#[cfg(test)]
mod tests;
