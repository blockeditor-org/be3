use block_editor_beui::beui::{CursorIcon, Key, KeyPress, Modifiers, PointerPress};
use block_editor_beui::reactive::{Get, untrack};
use infinite_canvas_types::{
    CanvasArrowhead, CanvasEntity, CanvasPoint, CanvasProfile, CanvasShape, CanvasTransform,
};
use uuid::Uuid;

use crate::freehand::Ink;
use crate::geometry::*;
use crate::raster::PixelTool;

use super::state::{CanvasCommand, CanvasState, Gesture, TextEntry, Tool};

impl CanvasState {
    pub(crate) fn cursor(&self) -> CursorIcon {
        match self.tool.get() {
            Tool::Text => CursorIcon::Text,
            Tool::Select | Tool::Lasso => self.select_cursor(),
            _ => CursorIcon::Crosshair,
        }
    }

    fn select_cursor(&self) -> CursorIcon {
        let Some(world) = self.pointer.get() else {
            return CursorIcon::Default;
        };
        if let Some(path) = self.selected_path() {
            return match path_grip_at(&path, world, self.scale(), HANDLE_REACH) {
                Some(_) => CursorIcon::Crosshair,
                None => CursorIcon::Default,
            };
        }
        let Some(frame) = selection_frame_of(self) else {
            return CursorIcon::Default;
        };
        if !self.selection_has_unlocked() {
            return CursorIcon::Default;
        }
        let scale = self.scale();
        if self.selection_allows_rotation()
            && rotate_handle_at(frame, world, scale, HANDLE_REACH).is_some()
        {
            return CursorIcon::Grab;
        }
        let (resize, _) = self.selection_handles();
        match resize_handle_at(frame, world, scale, resize, HANDLE_REACH) {
            Some((handle, _)) => match (handle.x, handle.y) {
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
            .embedding_untracked()
            .into_iter()
            .rev()
            .filter_map(|id| self.entities.get_untracked(id))
            .find_map(|entity| {
                (entity.shape == CanvasShape::Editor
                    && !entity.hidden
                    && !self.shows_preview(entity.id)
                    && direct_editor_layout(&entity)
                        .is_some_and(|layout| layout.content.contains(world)))
                .then_some(entity.id)
            })
    }

    pub(crate) fn press(&self, press: PointerPress) {
        if self.previewing() {
            return;
        }
        self.note_touch(press.touch);
        if press.pen.is_some() {
            self.note_pen();
        }
        let world = self.world_at(press.pos);
        self.hover(Some(world));
        if let Some(child) = self.live_child_at(world) {
            if self.takes_frame(child) {
                self.focus_editor(Some(child));
            }
            return;
        }
        self.hold_pointer(true);
        if let Some(focused) = self.focused_editor.get_untracked() {
            let inside = self
                .entities
                .get_untracked(focused)
                .is_some_and(|entity| entity_bounds(&entity).contains(world));
            if !inside {
                self.focus_editor(None);
            }
        }
        let tool = match press.pen.is_some_and(|pen| pen.eraser) {
            true if self.profile.get_untracked() == CanvasProfile::Raster => {
                Tool::Pixel(PixelTool::Eraser)
            }
            true => Tool::Eraser,
            false => self.tool_for(press),
        };
        if press.clicks >= 2 && tool.selects() && self.double_press(press, world) {
            self.begin_gesture(None);
            return;
        }
        match tool {
            Tool::Select | Tool::Lasso => self.press_select(press, world, tool == Tool::Lasso),
            Tool::Line
            | Tool::Arrow
            | Tool::Rectangle
            | Tool::Ellipse
            | Tool::Diamond
            | Tool::Text
            | Tool::Frame => {
                self.begin_gesture(Some(Gesture::Create {
                    id: Uuid::new_v4(),
                    tool,
                    start: world,
                    current: world,
                    pointer: world,
                    from_center: press.modifiers.alt,
                    detached: press.modifiers.ctrl,
                    snapped: self.snaps.get_untracked(),
                }));
            }
            Tool::Pen => {
                let ink = press.pen.is_none().then(|| Ink::new(press.pos, press.time));
                self.begin_gesture(Some(Gesture::Pen {
                    id: Uuid::new_v4(),
                    points: vec![world],
                    pressures: vec![ink.map_or_else(|| pen_pressure(press), |ink| ink.pressure())],
                    predicted: None,
                    ink,
                }));
            }
            Tool::Eraser => self.begin_erasing(world),
            Tool::Pixel(pixel) => self.press_raster(pixel, world, press.modifiers.shift),
        }
    }

    fn double_press(&self, press: PointerPress, world: CanvasPoint) -> bool {
        let hit = self
            .entity_at(world)
            .and_then(|id| self.entities.get_untracked(id));
        if let Some(entity) = &hit
            && entity.group_id.is_some()
            && entity.group_id != self.entered_group.get_untracked()
        {
            self.enter_group(entity.id);
            return true;
        }
        match self.written_at(world) {
            Some(entity)
                if entity.locked || (entity.shape != CanvasShape::Text && press.clicks != 2) =>
            {
                false
            }
            Some(entity) => {
                self.select(entity.id, false);
                self.edit_text_at(entity.id, TextEntry::At(press.pos));
                true
            }
            None if press.clicks == 2 && hit.is_none() => {
                self.write_at(world);
                true
            }
            None => false,
        }
    }

    fn tool_for(&self, press: PointerPress) -> Tool {
        match self.tool.get_untracked() {
            Tool::Pen if press.touch && self.pen_seen() => Tool::Select,
            tool => tool,
        }
    }

    fn press_select(&self, press: PointerPress, world: CanvasPoint, lasso: bool) {
        if let Some(path) = self.selected_path() {
            let reach = match press.touch {
                true => TOUCH_HANDLE_REACH,
                false => HANDLE_REACH,
            };
            if let Some(grip) = path_grip_at(&path, world, self.scale(), reach) {
                self.grip_path(path, grip, world, press.modifiers);
                return;
            }
        }
        let frame = selection_frame_of(self);
        let has_unlocked = self.selection_has_unlocked() && self.selected_path().is_none();
        let (resize, scale_editors) = self.selection_handles();
        let scale = self.scale();
        let reach = frame.map_or(HANDLE_REACH, |frame| {
            handle_reach(frame, scale, press.touch)
        });
        let handle = has_unlocked
            .then_some(frame)
            .flatten()
            .and_then(|frame| resize_handle_at(frame, world, scale, resize, reach));
        let rotate = has_unlocked
            && self.selection_allows_rotation()
            && frame
                .and_then(|frame| rotate_handle_at(frame, world, scale, reach))
                .is_some_and(|away| handle.is_none_or(|(_, nearest)| away < nearest));
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
        if let (Some(frame), Some((handle, _))) = (frame, handle) {
            let default_preserve_aspect_ratio = self.selection_defaults_to_proportional();
            let scale_text = press.modifiers.alt;
            let force_preserve_aspect_ratio = self.selection_forces_proportional() || scale_editors;
            let preserve_aspect_ratio = scale_text
                || force_preserve_aspect_ratio
                || default_preserve_aspect_ratio != press.modifiers.shift;
            let at = frame.point(CanvasPoint::new(
                handle.x as f32 * 0.5,
                handle.y as f32 * 0.5,
            ));
            self.begin_gesture(Some(Gesture::Resize {
                handle,
                frame,
                current: at,
                grab: CanvasPoint::new(at.x - world.x, at.y - world.y),
                originals: self.selected_unlocked(),
                default_preserve_aspect_ratio,
                force_preserve_aspect_ratio,
                preserve_aspect_ratio,
                scale_text,
                scale_editors,
                snapped: self.snaps.get_untracked(),
            }));
            return;
        }
        if let Some(id) = self.entity_at(world) {
            let entity = self
                .entities
                .get_untracked(id)
                .expect("the hit entity is one of the canvas entities");
            if entity.shape == CanvasShape::Editor {
                let content = direct_editor_layout(&entity)
                    .is_some_and(|layout| layout.content.contains(world));
                if content && self.takes_frame(entity.id) {
                    self.focus_editor(Some(id));
                    self.begin_gesture(None);
                    return;
                }
            }
            let retap = press.touch
                && !entity.locked
                && entity.shape == CanvasShape::Text
                && self.selection.get_untracked().len() == 1
                && self.selection.get_untracked().contains(&id);
            self.note_retap(retap.then_some((id, press.pos)));
            if entity.group_id != self.entered_group.get_untracked() {
                self.leave_group();
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
        self.leave_group();
        self.begin_gesture(Some(match lasso {
            true => Gesture::Lasso {
                points: vec![world],
                additive: press.modifiers.shift,
            },
            false => Gesture::SelectBox {
                start: world,
                current: world,
                pointer: world,
                from_center: press.modifiers.alt,
                additive: press.modifiers.shift,
            },
        }));
    }

    fn grip_path(
        &self,
        path: CanvasEntity,
        grip: PathGrip,
        world: CanvasPoint,
        modifiers: Modifiers,
    ) {
        let last = path.points.len().saturating_sub(1);
        let (path, index) = match grip {
            PathGrip::Point(index) => (path, index),
            PathGrip::Middle(index) => (split_segment(path, index), index + 1),
        };
        let at = path
            .world_points()
            .get(index)
            .map_or(world, |point| point.at());
        let grab = CanvasPoint::new(at.x - world.x, at.y - world.y);
        let original = Box::new(path);
        self.begin_gesture(Some(match grip {
            PathGrip::Point(index) if index == 0 || index == last => Gesture::Endpoint {
                original,
                end: usize::from(index != 0),
                current: at,
                grab,
                snap: modifiers.shift,
                detached: modifiers.ctrl,
                snapped: self.snaps.get_untracked(),
            },
            _ => Gesture::Bend {
                original,
                index,
                current: at,
                grab,
                snapped: self.snaps.get_untracked(),
            },
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
        match self.gesture.get_untracked() {
            Some(Gesture::Erase { .. }) => {
                self.erase_toward(world, modifiers.alt);
                return;
            }
            Some(Gesture::Raster { .. }) => {
                self.drag_raster(world, modifiers.shift);
                return;
            }
            _ => {}
        }
        let scale = self.scale();
        let pen_step = 1.0 / scale;
        let predicted = press.predicted.map(|at| self.world_at(at));
        self.update_gesture(|gesture| match gesture {
            Gesture::Create {
                tool,
                start,
                current,
                pointer,
                from_center,
                detached,
                ..
            } => {
                *detached = modifiers.ctrl;
                if tool.draws_a_box() {
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
                    *current = match tool.draws_a_line() && modifiers.shift {
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
            Gesture::Lasso { points, .. } => {
                if points
                    .last()
                    .is_none_or(|last| distance(*last, world) * scale >= LASSO_STEP)
                {
                    points.push(world);
                }
            }
            Gesture::Move { current, .. } => *current = world,
            Gesture::Bend { current, grab, .. } => {
                *current = CanvasPoint::new(world.x + grab.x, world.y + grab.y);
            }
            Gesture::Endpoint {
                current,
                grab,
                snap,
                detached,
                ..
            } => {
                *current = CanvasPoint::new(world.x + grab.x, world.y + grab.y);
                *snap = modifiers.shift;
                *detached = modifiers.ctrl;
            }
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
                grab,
                default_preserve_aspect_ratio,
                force_preserve_aspect_ratio,
                preserve_aspect_ratio,
                scale_text,
                ..
            } => {
                *current = CanvasPoint::new(world.x + grab.x, world.y + grab.y);
                *scale_text = modifiers.alt;
                *preserve_aspect_ratio = *scale_text
                    || *force_preserve_aspect_ratio
                    || *default_preserve_aspect_ratio != modifiers.shift;
            }
            Gesture::Erase { .. } | Gesture::Raster { .. } => {}
            Gesture::Pen {
                points,
                pressures,
                predicted: tail,
                ink,
                ..
            } => {
                *tail = predicted;
                let pressure = match ink {
                    Some(ink) => ink.sample(press.pos, press.time),
                    None => pen_pressure(press),
                };
                if points
                    .last()
                    .is_none_or(|last| distance(*last, world) > pen_step)
                {
                    points.push(world);
                    pressures.push(pressure);
                }
            }
        });
    }

    pub(crate) fn release(&self) {
        if self.touching() {
            self.hover(None);
        }
        self.hold_pointer(false);
        let gesture = self.take_gesture();
        let retapped = self.take_retap();
        if let (Some((id, at)), Some(Gesture::Move { start, current, .. })) = (retapped, &gesture)
            && start == current
        {
            self.commit_writes();
            self.edit_text_at(id, TextEntry::At(at));
            return;
        }
        if let Some(gesture) = gesture {
            self.finish_gesture(gesture);
        }
        self.commit_writes();
    }

    pub(crate) fn cancel(&self) {
        self.begin_gesture(None);
    }

    pub(crate) fn key(&self, press: KeyPress) -> bool {
        if self.previewing() || !press.pressed {
            return false;
        }
        if press.key == Key::Escape {
            if matches!(
                self.gesture.get_untracked(),
                Some(Gesture::Erase { .. } | Gesture::Raster { .. })
            ) {
                self.begin_gesture(None);
            } else if self.focused_editor.get_untracked().is_some() {
                self.focus_editor(None);
            } else if let Some(group) = self.leave_group() {
                self.select_group(group);
            } else {
                self.begin_gesture(None);
                self.put_down_tool();
            }
            return true;
        }
        if self.focused_editor.get_untracked().is_some() {
            return false;
        }
        let modifiers = press.modifiers;
        let nudge = match press.key {
            Key::ArrowLeft => Some((-1.0, 0.0)),
            Key::ArrowRight => Some((1.0, 0.0)),
            Key::ArrowUp => Some((0.0, -1.0)),
            Key::ArrowDown => Some((0.0, 1.0)),
            _ => None,
        };
        if let Some((x, y)) = nudge {
            let before = self.selected_unlocked();
            let amount = match modifiers.shift {
                true => 10.0,
                false => 1.0,
            };
            let snapped = self.snaps.get_untracked();
            let after = before
                .iter()
                .cloned()
                .map(|mut entity| {
                    entity.transform.center.x += x * amount;
                    entity.transform.center.y += y * amount;
                    match snapped {
                        true => super::state::moved_to_whole_units(entity),
                        false => entity,
                    }
                })
                .collect();
            self.record_update(before, after);
            return true;
        }
        if press.key == Key::Enter && self.selection.get_untracked().len() == 1 {
            self.edit_selected();
            return true;
        }
        if matches!(press.key, Key::Delete | Key::Backspace)
            && self.tool.get_untracked().selects()
            && !self.selection.get_untracked().is_empty()
        {
            self.run(CanvasCommand::Delete);
            return true;
        }
        false
    }

    pub(crate) fn add_at(&self, tool: Tool, center: CanvasPoint) {
        let boxed = |shape: CanvasShape, size: CanvasPoint| {
            CanvasEntity::new(shape, CanvasTransform::new(center, size, 0.0))
        };
        let line = || {
            CanvasEntity::path(&[
                CanvasPoint::new(center.x - 90.0, center.y),
                CanvasPoint::new(center.x + 90.0, center.y),
            ])
        };
        let entity = match tool {
            Tool::Rectangle => boxed(CanvasShape::Rectangle, CanvasPoint::new(180.0, 100.0)),
            Tool::Ellipse => boxed(CanvasShape::Ellipse, CanvasPoint::new(180.0, 100.0)),
            Tool::Diamond => boxed(CanvasShape::Diamond, CanvasPoint::new(180.0, 100.0)),
            Tool::Line | Tool::Arrow => line(),
            Tool::Text => {
                let mut text = boxed(CanvasShape::Text, CanvasPoint::new(180.0, 36.0));
                text.placeholder = "Text".into();
                text
            }
            Tool::Select
            | Tool::Lasso
            | Tool::Pen
            | Tool::Eraser
            | Tool::Frame
            | Tool::Pixel(_) => {
                self.set_tool(tool);
                return;
            }
        };
        let mut entity = self.styled(entity);
        if tool == Tool::Arrow {
            entity.style.end_arrowhead = CanvasArrowhead::Arrow;
        }
        self.add_entity(entity);
        self.put_down_tool();
    }
}

const LASSO_STEP: f32 = 4.0;

fn selection_frame_of(state: &CanvasState) -> Option<SelectionFrame> {
    super::state::selection_frame(
        &untrack(|| state.selected_entities()),
        &state.selection.get_untracked(),
    )
}

fn pen_pressure(press: PointerPress) -> f32 {
    press
        .pen
        .map_or(PEN_PRESSURE, |pen| pen.pressure)
        .clamp(0.0, 1.0)
}
