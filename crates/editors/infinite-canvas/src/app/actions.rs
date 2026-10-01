use std::rc::Rc;

use block_editor_beui::be_block::canvas::{CanvasEntityKind, CanvasLayerMove};
use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_CONTENT_COPY, ICON_CONTENT_CUT,
    ICON_CONTENT_PASTE, ICON_CROP_FREE, ICON_DATA_OBJECT, ICON_DELETE, ICON_DESELECT,
    ICON_DIAGONAL_LINE, ICON_DRAW, ICON_EDIT, ICON_FILTER_CENTER_FOCUS, ICON_FIT_SCREEN,
    ICON_FLIP_TO_BACK, ICON_FLIP_TO_FRONT, ICON_GROUP_WORK, ICON_IMAGE, ICON_LIBRARY_ADD,
    ICON_LOCK, ICON_LOCK_OPEN, ICON_OPEN_IN_NEW, ICON_RECTANGLE, ICON_SELECT, ICON_SELECT_ALL,
    ICON_SWAP_HORIZ, ICON_TEXT_FIELDS, ICON_WORKSPACES, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::{Action, Chord, clone, create_memo};
use uuid::Uuid;

use super::state::{CanvasCommand, CanvasState, Tool, ZOOM_STEP};

#[derive(Clone)]
pub(crate) struct CanvasActions {
    pub(crate) select: Action,
    pub(crate) artboard: Action,
    pub(crate) line: Action,
    pub(crate) rectangle: Action,
    pub(crate) text: Action,
    pub(crate) pen: Action,
    pub(crate) add_rectangle: Action,
    pub(crate) add_line: Action,
    pub(crate) add_text: Action,
    pub(crate) add_image: Action,
    pub(crate) add_block: Action,
    pub(crate) edit: Action,
    pub(crate) open: Action,
    pub(crate) toggle_mode: Action,
    pub(crate) fit_selection: Action,
    pub(crate) cut: Action,
    pub(crate) copy: Action,
    pub(crate) paste: Action,
    pub(crate) duplicate: Action,
    pub(crate) delete: Action,
    pub(crate) group: Action,
    pub(crate) ungroup: Action,
    pub(crate) lock: Action,
    pub(crate) unlock: Action,
    pub(crate) front: Action,
    pub(crate) forward: Action,
    pub(crate) backward: Action,
    pub(crate) back: Action,
    pub(crate) select_all: Action,
    pub(crate) invert: Action,
    pub(crate) zoom_in: Action,
    pub(crate) zoom_out: Action,
    pub(crate) zoom_reset: Action,
    pub(crate) fit_all: Action,
}

impl CanvasActions {
    pub(crate) fn tool(&self, tool: Tool) -> Action {
        match tool {
            Tool::Select => self.select.clone(),
            Tool::Artboard => self.artboard.clone(),
            Tool::Line => self.line.clone(),
            Tool::Rectangle => self.rectangle.clone(),
            Tool::Text => self.text.clone(),
            Tool::Pen => self.pen.clone(),
        }
    }
}

pub(crate) fn canvas_actions(state: &Rc<CanvasState>) -> CanvasActions {
    let live = !state.previewing();
    let free = create_memo(clone!(state -> move || live && state.focused_editor.get().is_none()));
    let when = |test: fn(&CanvasState) -> bool| {
        create_memo(clone!(state free -> move || free.get() && test(&state)))
    };
    let selected = when(|state| !state.selection.get().is_empty());
    let unlocked = when(|state| {
        state
            .selected_entities()
            .iter()
            .any(|entity| !entity.locked)
    });
    let locked = when(|state| state.selected_entities().iter().any(|entity| entity.locked));
    let single = when(|state| state.selection.get().len() == 1);
    let grouped = when(|state| {
        state
            .selected_entities()
            .iter()
            .any(|entity| entity.group_id.is_some())
    });
    let groupable = when(CanvasState::selection_can_group);
    let block = when(|state| embedded_block(state).is_some());
    let artboard = when(|state| state.first_artboard().is_some());
    let reorder = |movement: CanvasLayerMove| {
        create_memo(clone!(state free -> move || free.get() && state.can_reorder(movement)))
    };
    let tool = |id: &str, label: &'static str, glyph: &str, tool: Tool, key: Key| {
        Action::new(id, label, clone!(state -> move || state.set_tool(tool)))
            .glyph(glyph)
            .shortcut(Chord::key(key))
            .checked(create_memo(
                clone!(state -> move || state.tool.get() == tool),
            ))
            .enabled(free.clone())
    };
    let command = |id: &str, label: &'static str, glyph: &str, command: CanvasCommand| {
        Action::new(id, label, clone!(state -> move || state.run(command))).glyph(glyph)
    };
    let add = |id: &str, label: &'static str, glyph: &str, tool: Tool| {
        Action::new(
            id,
            label,
            clone!(state -> move || {
                let at = state
                    .take_context_position()
                    .unwrap_or_else(|| state.viewport_center());
                state.add_at(tool, at);
            }),
        )
        .glyph(glyph)
        .enabled(free.clone())
    };
    let layer =
        |id: &str, label: &'static str, glyph: &str, movement: CanvasLayerMove, chord: Chord| {
            Action::new(id, label, clone!(state -> move || state.reorder(movement)))
                .glyph(glyph)
                .shortcut(chord)
                .enabled(reorder(movement))
        };
    let scale = state.editor().scale();
    let cut = command("canvas.cut", "Cut", ICON_CONTENT_CUT, CanvasCommand::Cut)
        .shortcut(Chord::ctrl(Key::X))
        .enabled(selected.clone())
        .in_menu()
        .register();
    let copy = command(
        "canvas.copy",
        "Copy",
        ICON_CONTENT_COPY,
        CanvasCommand::Copy,
    )
    .shortcut(Chord::ctrl(Key::C))
    .enabled(selected.clone())
    .in_menu()
    .register();
    let paste = command(
        "canvas.paste",
        "Paste",
        ICON_CONTENT_PASTE,
        CanvasCommand::Paste,
    )
    .shortcut(Chord::ctrl(Key::V))
    .enabled(free.clone())
    .in_menu()
    .register();
    let duplicate = command(
        "canvas.duplicate",
        "Duplicate",
        ICON_LIBRARY_ADD,
        CanvasCommand::Duplicate,
    )
    .shortcut(Chord::ctrl(Key::D))
    .enabled(selected.clone())
    .in_menu()
    .register();
    let delete = command(
        "canvas.delete",
        "Delete",
        ICON_DELETE,
        CanvasCommand::Delete,
    )
    .enabled(unlocked.clone())
    .in_menu()
    .register();
    let group = command(
        "canvas.group",
        "Group",
        ICON_GROUP_WORK,
        CanvasCommand::Group,
    )
    .shortcut(Chord::ctrl(Key::G))
    .enabled(groupable)
    .in_menu()
    .register();
    let ungroup = command(
        "canvas.ungroup",
        "Ungroup",
        ICON_WORKSPACES,
        CanvasCommand::Ungroup,
    )
    .shortcut(Chord::ctrl(Key::G).shift())
    .enabled(grouped)
    .in_menu()
    .register();
    let lock = command("canvas.lock", "Lock", ICON_LOCK, CanvasCommand::Lock)
        .enabled(unlocked)
        .in_menu()
        .register();
    let unlock = command(
        "canvas.unlock",
        "Unlock",
        ICON_LOCK_OPEN,
        CanvasCommand::Unlock,
    )
    .enabled(locked)
    .in_menu()
    .register();
    let select_all = command(
        "canvas.select-all",
        "Select all",
        ICON_SELECT_ALL,
        CanvasCommand::SelectAll,
    )
    .shortcut(Chord::ctrl(Key::A))
    .enabled(free.clone())
    .in_menu()
    .register();
    let invert = command(
        "canvas.invert-selection",
        "Invert selection",
        ICON_DESELECT,
        CanvasCommand::InvertSelection,
    )
    .shortcut(Chord::ctrl(Key::A).shift())
    .enabled(free.clone())
    .in_menu()
    .register();
    let fit_selection = Action::new(
        "canvas.fit-selection",
        "Fit selection",
        clone!(state -> move || state.request_fit_selection()),
    )
    .glyph(ICON_FILTER_CENTER_FOCUS)
    .enabled(selected.clone())
    .in_menu()
    .register();
    Action::new(
        "canvas.fit-artboard",
        "Fit the first artboard",
        clone!(state -> move || state.request_fit_artboard()),
    )
    .glyph(ICON_FIT_SCREEN)
    .enabled(artboard)
    .in_menu()
    .register();
    let select = tool(
        "canvas.tool.select",
        "Select",
        ICON_SELECT,
        Tool::Select,
        Key::V,
    )
    .register();
    let artboard = tool(
        "canvas.tool.artboard",
        "Draw an artboard",
        ICON_CROP_FREE,
        Tool::Artboard,
        Key::F,
    )
    .in_menu()
    .register();
    let line = tool(
        "canvas.tool.line",
        "Draw a line",
        ICON_DIAGONAL_LINE,
        Tool::Line,
        Key::L,
    )
    .in_menu()
    .register();
    let rectangle = tool(
        "canvas.tool.rectangle",
        "Draw a rectangle",
        ICON_RECTANGLE,
        Tool::Rectangle,
        Key::R,
    )
    .register();
    let text = tool(
        "canvas.tool.text",
        "Write text",
        ICON_TEXT_FIELDS,
        Tool::Text,
        Key::T,
    )
    .register();
    let pen = tool(
        "canvas.tool.pen",
        "Draw freehand",
        ICON_DRAW,
        Tool::Pen,
        Key::P,
    )
    .register();
    let add_rectangle = add(
        "canvas.add.rectangle",
        "Add a rectangle",
        ICON_RECTANGLE,
        Tool::Rectangle,
    )
    .register();
    let add_line = add(
        "canvas.add.line",
        "Add a line",
        ICON_DIAGONAL_LINE,
        Tool::Line,
    )
    .register();
    let add_text = add("canvas.add.text", "Add text", ICON_TEXT_FIELDS, Tool::Text).register();
    let add_image = Action::new(
        "canvas.add.image",
        "Add an image…",
        clone!(state -> move || state.open_image_picker(Some(
            state.take_context_position().unwrap_or_else(|| state.viewport_center()),
        ))),
    )
    .glyph(ICON_IMAGE)
    .enabled(free.clone())
    .register();
    let add_block = Action::new(
        "canvas.add.block",
        "Add a block…",
        clone!(state -> move || state.open_block_picker(state.take_context_position())),
    )
    .glyph(ICON_DATA_OBJECT)
    .enabled(free.clone())
    .register();
    let edit = Action::new(
        "canvas.edit",
        "Open / edit",
        clone!(state -> move || state.edit_selected()),
    )
    .glyph(ICON_EDIT)
    .enabled(single)
    .register();
    let open = Action::new(
        "canvas.open",
        "Open the block",
        clone!(state -> move || {
            if let Some(block) = embedded_block(&state) {
                state.open_referenced_block(block);
            }
        }),
    )
    .glyph(ICON_OPEN_IN_NEW)
    .enabled(block.clone())
    .register();
    let toggle_mode = Action::new(
        "canvas.toggle-mode",
        "Toggle preview / direct editor",
        clone!(state -> move || state.toggle_selected_block_mode()),
    )
    .glyph(ICON_SWAP_HORIZ)
    .enabled(block)
    .register();
    let front = layer(
        "canvas.bring-to-front",
        "Bring to front",
        ICON_FLIP_TO_FRONT,
        CanvasLayerMove::BringToFront,
        Chord::ctrl(Key::BracketRight).shift(),
    )
    .register();
    let forward = layer(
        "canvas.forward",
        "Forward",
        ICON_ARROW_UPWARD,
        CanvasLayerMove::ForwardOne,
        Chord::ctrl(Key::BracketRight),
    )
    .register();
    let backward = layer(
        "canvas.backward",
        "Backward",
        ICON_ARROW_DOWNWARD,
        CanvasLayerMove::BackOne,
        Chord::ctrl(Key::BracketLeft),
    )
    .register();
    let back = layer(
        "canvas.send-to-back",
        "Send to back",
        ICON_FLIP_TO_BACK,
        CanvasLayerMove::SendToBack,
        Chord::ctrl(Key::BracketLeft).shift(),
    )
    .register();
    let zoom_in = Action::new(
        "canvas.zoom-in",
        "Zoom in",
        clone!(state -> move || state.editor().zoom(ZOOM_STEP)),
    )
    .glyph(ICON_ZOOM_IN)
    .enabled(live)
    .register();
    let zoom_out = Action::new(
        "canvas.zoom-out",
        "Zoom out",
        clone!(state -> move || state.editor().zoom(1.0 / ZOOM_STEP)),
    )
    .glyph(ICON_ZOOM_OUT)
    .enabled(live)
    .register();
    let zoom_reset = Action::new(
        "canvas.zoom-reset",
        "Zoom to 100%",
        clone!(state scale -> move || {
            state.editor().zoom(1.0 / scale.get_untracked().max(f32::EPSILON));
        }),
    )
    .enabled(live)
    .register();
    let fit_all = Action::new(
        "canvas.fit-all",
        "Fit all",
        clone!(state -> move || state.editor().fit()),
    )
    .glyph(ICON_FIT_SCREEN)
    .enabled(live)
    .register();
    CanvasActions {
        select,
        artboard,
        line,
        rectangle,
        text,
        pen,
        add_rectangle,
        add_line,
        add_text,
        add_image,
        add_block,
        edit,
        open,
        toggle_mode,
        fit_selection,
        cut,
        copy,
        paste,
        duplicate,
        delete,
        group,
        ungroup,
        lock,
        unlock,
        front,
        forward,
        backward,
        back,
        select_all,
        invert,
        zoom_in,
        zoom_out,
        zoom_reset,
        fit_all,
    }
}

pub(crate) fn embedded_block(state: &CanvasState) -> Option<Uuid> {
    let selected = state.selected_entities();
    let [entity] = selected.as_slice() else {
        return None;
    };
    match entity.kind {
        CanvasEntityKind::Block { block_id } | CanvasEntityKind::DirectEditor { block_id, .. } => {
            Some(block_id)
        }
        _ => None,
    }
}
