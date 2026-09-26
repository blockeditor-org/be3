use block_editor_beui::InteractionMode;
use block_editor_beui::be_block::canvas::{
    CanvasEntityKind, CanvasLayerMove, CanvasPoint, CanvasTextStyle, CanvasTransform,
};
use block_editor_beui::beui::{CursorIcon, Key, KeyPress, PointerPress};
use uuid::Uuid;

use crate::geometry::*;

use super::state::{CanvasCommand, CanvasState, Gesture, Tool};

impl CanvasState {
    pub(crate) fn cursor(&self) -> CursorIcon {
        match self.tool.get() {
            Tool::Line | Tool::Rectangle | Tool::Pen => CursorIcon::Crosshair,
            Tool::Text => CursorIcon::Text,
            Tool::Select => self.select_cursor(),
        }
    }

    fn select_cursor(&self) -> CursorIcon {
        let Some(world) = self.pointer.get() else {
            return CursorIcon::Default;
        };
        let Some(frame) = selection_frame_of(self) else {
            return CursorIcon::Default;
        };
        if !self.selection_has_unlocked() {
            return CursorIcon::Default;
        }
        let scale = self.scale();
        if self.selection_allows_rotation() && rotate_handle_at(frame, world, scale) {
            return CursorIcon::Grab;
        }
        let (resize, _) = self.selection_handles();
        match resize_handle_at(frame, world, scale, resize) {
            Some(handle) => match (handle.x, handle.y) {
                (0, _) => CursorIcon::ResizeVertical,
                (_, 0) => CursorIcon::ResizeHorizontal,
                (x, y) if x == y => CursorIcon::ResizeNwSe,
                _ => CursorIcon::ResizeNeSw,
            },
            None => CursorIcon::Default,
        }
    }

    pub(crate) fn hover(&self, at: Option<CanvasPoint>) {
        self.note_pointer(at);
    }

    pub(crate) fn live_child_at(&self, world: CanvasPoint) -> Option<Uuid> {
        self.entities
            .get_untracked()
            .iter()
            .rev()
            .find_map(|entity| {
                (matches!(entity.kind, CanvasEntityKind::DirectEditor { .. })
                    && !self.shows_preview(entity.id)
                    && direct_editor_layout(entity)
                        .is_some_and(|layout| layout.content.contains(world)))
                .then_some(entity.id)
            })
    }

    pub(crate) fn press(&self, press: PointerPress) {
        if self.previewing() {
            return;
        }
        let world = self.world_at(press.pos);
        self.hover(Some(world));
        if let Some(child) = self.live_child_at(world) {
            if self.interaction(child) == Some(InteractionMode::Live) {
                self.focus_editor(Some(child));
            }
            return;
        }
        self.hold_pointer(true);
        if let Some(focused) = self.focused_editor.get_untracked() {
            let inside = self
                .entities
                .get_untracked()
                .iter()
                .find(|entity| entity.id == focused)
                .is_some_and(|entity| entity_bounds(entity).contains(world));
            if !inside {
                self.focus_editor(None);
            }
        }
        if press.clicks >= 2
            && self.tool.get_untracked() == Tool::Select
            && let Some(id) = self.entity_at(world)
        {
            let entities = self.entities.get_untracked();
            let entity = entities.iter().find(|entity| entity.id == id);
            if let Some(entity) = entity
                && matches!(entity.kind, CanvasEntityKind::Text { .. })
                && !entity.locked
            {
                self.select(id, false);
                self.edit_text(Some(id));
                self.begin_gesture(None);
                return;
            }
        }
        match self.tool.get_untracked() {
            Tool::Select => self.press_select(press, world),
            Tool::Line | Tool::Rectangle | Tool::Text => {
                self.begin_gesture(Some(Gesture::Create {
                    tool: self.tool.get_untracked(),
                    start: world,
                    current: world,
                    pointer: world,
                    from_center: press.modifiers.alt,
                }));
            }
            Tool::Pen => self.begin_gesture(Some(Gesture::Pen {
                points: vec![world],
            })),
        }
    }

    fn press_select(&self, press: PointerPress, world: CanvasPoint) {
        let frame = selection_frame_of(self);
        let has_unlocked = self.selection_has_unlocked();
        let (resize, scale_editors) = self.selection_handles();
        let scale = self.scale();
        let rotate = has_unlocked
            && self.selection_allows_rotation()
            && frame.is_some_and(|frame| rotate_handle_at(frame, world, scale));
        if let (true, Some(frame)) = (rotate, frame) {
            let center = frame.center;
            self.begin_gesture(Some(Gesture::Rotate {
                frame,
                start_angle: (world.y - center.y).atan2(world.x - center.x),
                current: world,
                originals: self.selected_unlocked(),
                snap_angle: press.modifiers.shift,
            }));
            return;
        }
        let handle = has_unlocked
            .then_some(frame)
            .flatten()
            .and_then(|frame| resize_handle_at(frame, world, scale, resize));
        if let (Some(frame), Some(handle)) = (frame, handle) {
            let default_preserve_aspect_ratio = self.selection_defaults_to_proportional();
            let scale_text = press.modifiers.alt;
            let force_preserve_aspect_ratio = self.selection_forces_proportional() || scale_editors;
            let preserve_aspect_ratio = scale_text
                || force_preserve_aspect_ratio
                || default_preserve_aspect_ratio != press.modifiers.shift;
            self.begin_gesture(Some(Gesture::Resize {
                handle,
                frame,
                current: world,
                originals: self.selected_unlocked(),
                default_preserve_aspect_ratio,
                force_preserve_aspect_ratio,
                preserve_aspect_ratio,
                scale_text,
                scale_editors,
            }));
            return;
        }
        if let Some(id) = self.entity_at(world) {
            let entities = self.entities.get_untracked();
            let entity = entities
                .iter()
                .find(|entity| entity.id == id)
                .expect("the hit entity is one of the canvas entities");
            if let CanvasEntityKind::DirectEditor { .. } = entity.kind {
                let content = direct_editor_layout(entity)
                    .is_some_and(|layout| layout.content.contains(world));
                let live = self.interaction(entity.id) == Some(InteractionMode::Live);
                if content && live {
                    self.focus_editor(Some(id));
                    self.begin_gesture(None);
                    return;
                }
            }
            if press.modifiers.shift {
                self.select(id, true);
                self.begin_gesture(None);
            } else {
                self.select(id, false);
                self.begin_move(world, press.modifiers.alt);
            }
            return;
        }
        if !self.selection.get_untracked().is_empty()
            && !press.modifiers.shift
            && frame.is_some_and(|frame| frame.contains(world))
        {
            self.begin_move(world, press.modifiers.alt);
            return;
        }
        self.begin_gesture(Some(Gesture::SelectBox {
            start: world,
            current: world,
            pointer: world,
            from_center: press.modifiers.alt,
            additive: press.modifiers.shift,
        }));
    }

    pub(crate) fn secondary_press(&self, press: PointerPress) {
        if self.previewing() {
            return;
        }
        let world = self.world_at(press.pos);
        self.note_context_position(Some(world));
        let frame = selection_frame_of(self);
        let inside = frame.is_some_and(|frame| frame.contains(world));
        if self.selection.get_untracked().is_empty() {
            if let Some(id) = self.entity_at(world) {
                self.select(id, false);
            }
        } else if !inside {
            self.clear_selection();
        }
    }

    pub(crate) fn drag(&self, press: PointerPress) {
        if self.previewing() || !self.pointer_held() {
            return;
        }
        let world = self.world_at(press.pos);
        self.hover(Some(world));
        let modifiers = press.modifiers;
        let pen_step = 1.0 / self.scale();
        self.update_gesture(|gesture| match gesture {
            Gesture::Create {
                tool,
                start,
                current,
                pointer,
                from_center,
            } => {
                if *tool == Tool::Rectangle {
                    let delta = CanvasPoint::new(world.x - pointer.x, world.y - pointer.y);
                    if modifiers.ctrl {
                        start.x += delta.x;
                        start.y += delta.y;
                    } else {
                        current.x += delta.x;
                        current.y += delta.y;
                    }
                    *pointer = world;
                    *from_center = modifiers.alt;
                } else {
                    *current = match *tool == Tool::Line && modifiers.shift {
                        true => constrain_point_angle(*start, world, std::f32::consts::FRAC_PI_4),
                        false => world,
                    };
                }
            }
            Gesture::SelectBox {
                start,
                current,
                pointer,
                from_center,
                ..
            } => {
                let delta = CanvasPoint::new(world.x - pointer.x, world.y - pointer.y);
                if modifiers.ctrl {
                    start.x += delta.x;
                    start.y += delta.y;
                } else {
                    current.x += delta.x;
                    current.y += delta.y;
                }
                *pointer = world;
                *from_center = modifiers.alt;
            }
            Gesture::Move { current, .. } => *current = world,
            Gesture::Rotate {
                current,
                snap_angle,
                ..
            } => {
                *current = world;
                *snap_angle = modifiers.shift;
            }
            Gesture::Resize {
                current,
                default_preserve_aspect_ratio,
                force_preserve_aspect_ratio,
                preserve_aspect_ratio,
                scale_text,
                ..
            } => {
                *current = world;
                *scale_text = modifiers.alt;
                *preserve_aspect_ratio = *scale_text
                    || *force_preserve_aspect_ratio
                    || *default_preserve_aspect_ratio != modifiers.shift;
            }
            Gesture::Pen { points } => {
                if points
                    .last()
                    .is_none_or(|last| distance(*last, world) > pen_step)
                {
                    points.push(world);
                }
            }
        });
    }

    pub(crate) fn release(&self) {
        self.hold_pointer(false);
        self.finish_grouped_edit();
        let gesture = self.gesture.get_untracked();
        self.begin_gesture(None);
        if let Some(gesture) = gesture {
            self.finish_gesture(gesture);
        }
    }

    pub(crate) fn key(&self, press: KeyPress) -> bool {
        if self.previewing() || !press.pressed {
            return false;
        }
        if press.key == Key::Escape {
            if self.focused_editor.get_untracked().is_some() {
                self.focus_editor(None);
            } else {
                self.begin_gesture(None);
                self.set_tool(Tool::Select);
            }
            return true;
        }
        if self.focused_editor.get_untracked().is_some() {
            return false;
        }
        let modifiers = press.modifiers;
        if !modifiers.ctrl && !modifiers.alt {
            let tool = match press.key {
                Key::V => Some(Tool::Select),
                Key::R => Some(Tool::Rectangle),
                Key::L => Some(Tool::Line),
                Key::T => Some(Tool::Text),
                Key::P => Some(Tool::Pen),
                _ => None,
            };
            if let Some(tool) = tool {
                self.set_tool(tool);
                return true;
            }
        }
        if modifiers.ctrl {
            let command = match press.key {
                Key::A if modifiers.shift => Some(CanvasCommand::InvertSelection),
                Key::A => Some(CanvasCommand::SelectAll),
                Key::D => Some(CanvasCommand::Duplicate),
                Key::C => Some(CanvasCommand::Copy),
                Key::X => Some(CanvasCommand::Cut),
                Key::V => Some(CanvasCommand::Paste),
                Key::BracketLeft => Some(CanvasCommand::Reorder(CanvasLayerMove::BackOne)),
                Key::BracketRight => Some(CanvasCommand::Reorder(CanvasLayerMove::ForwardOne)),
                _ => None,
            };
            if let Some(command) = command {
                self.run(command);
                return true;
            }
        }
        let nudge = match press.key {
            Key::ArrowLeft => Some((-1.0, 0.0)),
            Key::ArrowRight => Some((1.0, 0.0)),
            Key::ArrowUp => Some((0.0, -1.0)),
            Key::ArrowDown => Some((0.0, 1.0)),
            _ => None,
        };
        if let Some((x, y)) = nudge {
            let amount = match modifiers.shift {
                true => 10.0,
                false => 1.0,
            };
            let before = self.selected_unlocked();
            let after = before
                .iter()
                .cloned()
                .map(|mut entity| {
                    entity.transform.center.x += x * amount;
                    entity.transform.center.y += y * amount;
                    entity
                })
                .collect();
            self.record_update(before, after, true);
            return true;
        }
        if press.key == Key::Enter && self.selection.get_untracked().len() == 1 {
            self.edit_selected();
            return true;
        }
        if matches!(press.key, Key::Delete | Key::Backspace)
            && self.tool.get_untracked() == Tool::Select
            && !self.selection.get_untracked().is_empty()
        {
            self.run(CanvasCommand::Delete);
            return true;
        }
        false
    }

    pub(crate) fn add_at(&self, tool: Tool, center: CanvasPoint) {
        let style = self.default_style();
        let entity = match tool {
            Tool::Rectangle => Some((CanvasPoint::new(180.0, 100.0), CanvasEntityKind::Rectangle)),
            Tool::Line => Some((CanvasPoint::new(180.0, MIN_SIZE), CanvasEntityKind::Line)),
            Tool::Text => Some((
                CanvasPoint::new(180.0, 36.0),
                CanvasEntityKind::Text {
                    text: String::new(),
                    text_style: CanvasTextStyle::default(),
                    placeholder: "Text".into(),
                },
            )),
            Tool::Select | Tool::Pen => None,
        };
        let Some((size, kind)) = entity else {
            self.set_tool(tool);
            return;
        };
        self.add_entity(block_editor_beui::be_block::canvas::CanvasEntity {
            id: Uuid::new_v4(),
            transform: CanvasTransform::new(center, size, 0.0),
            kind,
            style,
            group_id: None,
            locked: false,
            components: Vec::new(),
        });
        self.set_tool(Tool::Select);
    }
}

fn selection_frame_of(state: &CanvasState) -> Option<SelectionFrame> {
    super::state::selection_frame(
        &state.entities.get_untracked(),
        &state.selection.get_untracked(),
    )
}
