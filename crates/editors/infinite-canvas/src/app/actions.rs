use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_ALIGN_HORIZONTAL_SPACE_BETWEEN, ICON_ALIGN_VERTICAL_SPACE_BETWEEN, ICON_ARROW_DOWN,
    ICON_ARROW_LEFT_RIGHT, ICON_ARROW_UP, ICON_BOXES, ICON_BRACES, ICON_CLIPBOARD, ICON_COMPONENT,
    ICON_COPY, ICON_COPY_PLUS, ICON_DOWNLOAD, ICON_EXTERNAL_LINK, ICON_FOCUS, ICON_GRID_2X2,
    ICON_IMAGE, ICON_LAYERS_ARROW_UP, ICON_LINE_SQUIGGLE, ICON_LINK, ICON_LOCK, ICON_LOCK_OPEN,
    ICON_MINUS, ICON_PAINT_BUCKET, ICON_PENCIL, ICON_PIN, ICON_PLUS, ICON_RECTANGLE_HORIZONTAL,
    ICON_SCAN, ICON_SCISSORS, ICON_SEND_TO_BACK, ICON_SLASH, ICON_SPLINE_POINTER,
    ICON_SQUARE_DASHED, ICON_SQUARE_DASHED_MOUSE_POINTER, ICON_TRASH, ICON_TYPE, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::use_theme;
use infinite_canvas_types::{CanvasLayerMove, CanvasProfile};
use uuid::Uuid;

use super::export::{ExportFormat, ExportTo, Exporter};
use super::grid::{GridActions, grid_actions};
use super::state::{CanvasCommand, CanvasState, Tool, ZOOM_STEP};
use super::touch::TouchChoice;
use crate::raster::PixelTool;

#[derive(Clone)]
pub(crate) struct CanvasActions {
    pub(crate) select: Action,
    pub(crate) lasso: Action,
    pub(crate) select_mode: Action,
    pub(crate) frame: Action,
    pub(crate) line: Action,
    pub(crate) arrow: Action,
    pub(crate) rectangle: Action,
    pub(crate) ellipse: Action,
    pub(crate) diamond: Action,
    pub(crate) text: Action,
    pub(crate) pen: Action,
    pub(crate) eraser: Action,
    pub(crate) tool_lock: Action,
    pub(crate) bind_lines: Action,
    pub(crate) add_rectangle: Action,
    pub(crate) add_line: Action,
    pub(crate) add_text: Action,
    pub(crate) add_image: Action,
    pub(crate) add_block: Action,
    pub(crate) edit: Action,
    pub(crate) open: Action,
    pub(crate) toggle_mode: Action,
    pub(crate) fit_selection: Action,
    pub(crate) fit_frame: Action,
    pub(crate) cut: Action,
    pub(crate) copy: Action,
    pub(crate) paste: Action,
    pub(crate) copy_png: Action,
    pub(crate) copy_svg: Action,
    pub(crate) export_png: Action,
    pub(crate) export_svg: Action,
    pub(crate) exporter: Exporter,
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
    pub(crate) ignore_touch: Action,
    pub(crate) touch: TouchChoice,
    pub(crate) grid: GridActions,
    pub(crate) prototyping: Action,
    pub(crate) vector: Action,
    pub(crate) raster_profile: Action,
    pub(crate) pixel: Vec<Action>,
    pub(crate) raster: RasterActions,
}

#[derive(Clone)]
pub(crate) struct RasterActions {
    pub(crate) larger: Action,
    pub(crate) smaller: Action,
    pub(crate) filled: Action,
    pub(crate) mirror_horizontal: Action,
    pub(crate) mirror_vertical: Action,
    pub(crate) grid: Action,
}

impl RasterActions {
    pub(crate) fn all(&self) -> [&Action; 6] {
        [
            &self.larger,
            &self.smaller,
            &self.filled,
            &self.mirror_horizontal,
            &self.mirror_vertical,
            &self.grid,
        ]
    }
}

impl CanvasActions {
    pub(crate) fn tool(&self, tool: Tool) -> Action {
        match tool {
            Tool::Select => self.select.clone(),
            Tool::Lasso => self.lasso.clone(),
            Tool::Frame => self.frame.clone(),
            Tool::Line => self.line.clone(),
            Tool::Arrow => self.arrow.clone(),
            Tool::Rectangle => self.rectangle.clone(),
            Tool::Ellipse => self.ellipse.clone(),
            Tool::Diamond => self.diamond.clone(),
            Tool::Text => self.text.clone(),
            Tool::Pen => self.pen.clone(),
            Tool::Eraser => self.eraser.clone(),
            Tool::Pixel(pixel) => self.pixel[PixelTool::ALL
                .iter()
                .position(|tool| *tool == pixel)
                .expect("every pixel tool is listed")]
            .clone(),
        }
    }

    pub(crate) fn pixel_tools(&self) -> Vec<&Action> {
        std::iter::once(&self.select)
            .chain(self.pixel.iter())
            .collect()
    }

    pub(crate) fn tools(&self) -> [&Action; 11] {
        [
            &self.select,
            &self.lasso,
            &self.rectangle,
            &self.diamond,
            &self.ellipse,
            &self.arrow,
            &self.line,
            &self.pen,
            &self.text,
            &self.eraser,
            &self.frame,
        ]
    }
}

pub(crate) fn canvas_actions(state: CanvasState) -> CanvasActions {
    let live = !state.previewing();
    let free = create_memo(move || live && state.focused_editor.get().is_none());
    let when = |test: fn(&CanvasState) -> bool| create_memo(move || free.get() && test(&state));
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
    let framed = when(|state| state.first_frame().is_some());
    let reorder =
        |movement: CanvasLayerMove| create_memo(move || free.get() && state.can_reorder(movement));
    let tool = |id: &str, label: &'static str, tool: Tool| {
        Action::new(id, label, move || state.set_tool(tool))
            .glyph(tool.glyph())
            .radio(create_memo(move || state.tool.get() == tool))
            .enabled(create_memo(move || {
                free.get() && tool.usable_in(state.profile.get())
            }))
    };
    let rastering = create_memo(move || free.get() && state.profile.get() == CanvasProfile::Raster);
    let command = |id: &str, label: &'static str, glyph: Glyph, command: CanvasCommand| {
        Action::new(id, label, move || state.run(command)).glyph(glyph)
    };
    let add = |id: &str, label: &'static str, glyph: Glyph, tool: Tool| {
        Action::new(id, label, move || {
            let at = state
                .take_context_position()
                .unwrap_or_else(|| state.viewport_center());
            state.add_at(tool, at);
        })
        .glyph(glyph)
        .enabled(free)
    };
    let layer =
        |id: &str, label: &'static str, glyph: Glyph, movement: CanvasLayerMove, chord: Chord| {
            Action::new(id, label, move || state.reorder(movement))
                .glyph(glyph)
                .shortcut(chord)
                .enabled(reorder(movement))
        };
    let scale = state.editor().scale();
    let cut = command("canvas.cut", "Cut", ICON_SCISSORS, CanvasCommand::Cut)
        .shortcut(Chord::ctrl(Key::X))
        .enabled(selected)
        .register();
    let copy = command("canvas.copy", "Copy", ICON_COPY, CanvasCommand::Copy)
        .shortcut(Chord::ctrl(Key::C))
        .enabled(selected)
        .register();
    let paste = command(
        "canvas.paste",
        "Paste",
        ICON_CLIPBOARD,
        CanvasCommand::Paste,
    )
    .shortcut(Chord::ctrl(Key::V))
    .enabled(free)
    .register();
    let theme = use_theme();
    let exporter = Exporter::new(
        state,
        create_memo(move || super::canvas::palette(&theme)),
        create_memo(move || beui::reactive::resolve_color(theme.background.get())),
    );
    let drawn = when(|state| state.entities.ids().with(|ids| !ids.is_empty()));
    let export =
        |id: &str, label: &'static str, glyph: Glyph, format: ExportFormat, to: ExportTo| {
            Action::new(id, label, move || match to {
                ExportTo::Clipboard => exporter.run(format, to),
                ExportTo::File => exporter.ask(format),
            })
            .glyph(glyph)
            .enabled(drawn)
            .register()
        };
    let copy_png = export(
        "canvas.copy-png",
        "Copy as PNG",
        ICON_IMAGE,
        ExportFormat::Png,
        ExportTo::Clipboard,
    );
    let copy_svg = export(
        "canvas.copy-svg",
        "Copy as SVG",
        ICON_SPLINE_POINTER,
        ExportFormat::Svg,
        ExportTo::Clipboard,
    );
    let export_png = export(
        "canvas.export-png",
        "Export as PNG…",
        ICON_DOWNLOAD,
        ExportFormat::Png,
        ExportTo::File,
    );
    let export_svg = export(
        "canvas.export-svg",
        "Export as SVG…",
        ICON_DOWNLOAD,
        ExportFormat::Svg,
        ExportTo::File,
    );
    let duplicate = command(
        "canvas.duplicate",
        "Duplicate",
        ICON_COPY_PLUS,
        CanvasCommand::Duplicate,
    )
    .shortcut(Chord::ctrl(Key::D))
    .enabled(selected)
    .register();
    let delete = command("canvas.delete", "Delete", ICON_TRASH, CanvasCommand::Delete)
        .enabled(unlocked)
        .register();
    let group = command(
        "canvas.group",
        "Group",
        ICON_COMPONENT,
        CanvasCommand::Group,
    )
    .shortcut(Chord::ctrl(Key::G))
    .enabled(groupable)
    .register();
    let ungroup = command(
        "canvas.ungroup",
        "Ungroup",
        ICON_BOXES,
        CanvasCommand::Ungroup,
    )
    .shortcut(Chord::ctrl(Key::G).shift())
    .enabled(grouped)
    .register();
    let lock = command("canvas.lock", "Lock", ICON_LOCK, CanvasCommand::Lock)
        .enabled(unlocked)
        .register();
    let unlock = command(
        "canvas.unlock",
        "Unlock",
        ICON_LOCK_OPEN,
        CanvasCommand::Unlock,
    )
    .enabled(locked)
    .register();
    let select_all = command(
        "canvas.select-all",
        "Select all",
        ICON_SQUARE_DASHED_MOUSE_POINTER,
        CanvasCommand::SelectAll,
    )
    .shortcut(Chord::ctrl(Key::A))
    .enabled(free)
    .register();
    let invert = command(
        "canvas.invert-selection",
        "Invert selection",
        ICON_SQUARE_DASHED,
        CanvasCommand::InvertSelection,
    )
    .shortcut(Chord::ctrl(Key::A).shift())
    .enabled(free)
    .register();
    let fit_selection = Action::new("canvas.fit-selection", "Fit selection", move || {
        state.request_fit_selection()
    })
    .glyph(ICON_FOCUS)
    .enabled(selected)
    .register();
    let fit_frame = Action::new("canvas.fit-frame", "Fit the first frame", move || {
        state.request_fit_frame()
    })
    .glyph(ICON_SCAN)
    .enabled(framed)
    .register();
    let profile = |id: &str, label: &'static str, glyph: Glyph, profile: CanvasProfile| {
        Action::new(id, label, move || state.switch_profile(profile))
            .glyph(glyph)
            .radio(create_memo(move || state.profile.get() == profile))
            .enabled(live)
            .register()
    };
    let prototyping = profile(
        "canvas.profile.prototyping",
        "Prototyping profile",
        ICON_LINE_SQUIGGLE,
        CanvasProfile::Prototyping,
    );
    let vector = profile(
        "canvas.profile.vector",
        "Vector profile",
        ICON_SPLINE_POINTER,
        CanvasProfile::Vector,
    );
    let raster_profile = profile(
        "canvas.profile.raster",
        "Raster profile",
        ICON_GRID_2X2,
        CanvasProfile::Raster,
    );
    let pixel = PixelTool::ALL
        .into_iter()
        .map(|pixel| {
            let action = tool(
                &format!("canvas.tool.{}", pixel.id()),
                pixel.name(),
                Tool::Pixel(pixel),
            );
            let action = match pixel {
                PixelTool::Pencil => action
                    .shortcut(Chord::key(Key::B))
                    .shortcut(Chord::key(Key::P)),
                PixelTool::Eraser => action.shortcut(Chord::key(Key::E)),
                PixelTool::Fill => action.shortcut(Chord::key(Key::G)),
                PixelTool::Eyedropper => action.shortcut(Chord::key(Key::I)),
                PixelTool::ReplaceColor => action.shortcut(Chord::key(Key::C)),
                PixelTool::Line => action.shortcut(Chord::key(Key::L)),
                PixelTool::Rectangle => action.shortcut(Chord::key(Key::R)),
                PixelTool::Ellipse => action.shortcut(Chord::key(Key::O)),
            };
            action.register()
        })
        .collect();
    let tools = state.raster;
    let toggle = |signal: Signal<bool>| move || signal.update(|on| *on = !*on);
    let raster = RasterActions {
        larger: Action::new("canvas.brush.larger", "Larger brush", move || {
            tools.resize_brush(1)
        })
        .glyph(ICON_PLUS)
        .shortcut(Chord::key(Key::BracketRight))
        .enabled(rastering)
        .register(),
        smaller: Action::new("canvas.brush.smaller", "Smaller brush", move || {
            tools.resize_brush(-1)
        })
        .glyph(ICON_MINUS)
        .shortcut(Chord::key(Key::BracketLeft))
        .enabled(rastering)
        .register(),
        filled: Action::new(
            "canvas.brush.filled",
            "Fill pixel rectangles and ellipses",
            toggle(tools.filled),
        )
        .glyph(ICON_PAINT_BUCKET)
        .shortcut(Chord::key(Key::X))
        .checked(tools.filled)
        .enabled(rastering)
        .register(),
        mirror_horizontal: Action::new(
            "canvas.brush.mirror-horizontal",
            "Mirror left and right",
            toggle(tools.mirror_horizontal),
        )
        .glyph(ICON_ALIGN_HORIZONTAL_SPACE_BETWEEN)
        .shortcut(Chord::key(Key::H))
        .checked(tools.mirror_horizontal)
        .enabled(rastering)
        .register(),
        mirror_vertical: Action::new(
            "canvas.brush.mirror-vertical",
            "Mirror top and bottom",
            toggle(tools.mirror_vertical),
        )
        .glyph(ICON_ALIGN_VERTICAL_SPACE_BETWEEN)
        .shortcut(Chord::key(Key::H).shift())
        .checked(tools.mirror_vertical)
        .enabled(rastering)
        .register(),
        grid: Action::new(
            "canvas.pixel-grid",
            "Show the pixel grid",
            toggle(tools.grid),
        )
        .glyph(ICON_GRID_2X2)
        .checked(tools.grid)
        .enabled(rastering)
        .register(),
    };
    let select = tool("canvas.tool.select", "Select", Tool::Select)
        .shortcut(Chord::key(Key::V))
        .shortcut(Chord::key(Key::One))
        .register();
    let rectangle = tool("canvas.tool.rectangle", "Draw a rectangle", Tool::Rectangle)
        .shortcut(Chord::key(Key::R))
        .shortcut(Chord::key(Key::Two))
        .register();
    let diamond = tool("canvas.tool.diamond", "Draw a diamond", Tool::Diamond)
        .shortcut(Chord::key(Key::D))
        .shortcut(Chord::key(Key::Three))
        .register();
    let ellipse = tool("canvas.tool.ellipse", "Draw an ellipse", Tool::Ellipse)
        .shortcut(Chord::key(Key::O))
        .shortcut(Chord::key(Key::Four))
        .register();
    let arrow = tool("canvas.tool.arrow", "Draw an arrow", Tool::Arrow)
        .shortcut(Chord::key(Key::A))
        .shortcut(Chord::key(Key::Five))
        .register();
    let line = tool("canvas.tool.line", "Draw a line", Tool::Line)
        .shortcut(Chord::key(Key::L))
        .shortcut(Chord::key(Key::Six))
        .register();
    let pen = tool("canvas.tool.pen", "Draw freehand", Tool::Pen)
        .shortcut(Chord::key(Key::P))
        .shortcut(Chord::key(Key::Seven))
        .register();
    let text = tool("canvas.tool.text", "Write text", Tool::Text)
        .shortcut(Chord::key(Key::T))
        .shortcut(Chord::key(Key::Eight))
        .register();
    let eraser = tool("canvas.tool.eraser", "Erase", Tool::Eraser)
        .shortcut(Chord::key(Key::E))
        .shortcut(Chord::key(Key::Zero))
        .register();
    let lasso = tool("canvas.tool.lasso", "Lasso select", Tool::Lasso)
        .shortcut(Chord::key(Key::V).shift())
        .register();
    let select_mode = Action::new("canvas.tool.select-mode", "Select", move || {
        state.switch_select_tool()
    })
    .checked(create_memo(move || state.tool.get().selects()))
    .enabled(free)
    .detached();
    let frame = tool("canvas.tool.frame", "Draw a frame", Tool::Frame)
        .shortcut(Chord::key(Key::F))
        .register();
    let tool_lock = Action::new(
        "canvas.tool-lock",
        "Keep the tool after drawing",
        move || state.tool_locked.update(|locked| *locked = !*locked),
    )
    .glyph(ICON_PIN)
    .shortcut(Chord::key(Key::Q))
    .checked(state.tool_locked)
    .enabled(free)
    .register();
    let bind_lines = Action::new(
        "canvas.bind-lines",
        "Bind lines and arrows to shapes",
        move || state.binds.update(|binds| *binds = !*binds),
    )
    .glyph(ICON_LINK)
    .checked(state.binds)
    .enabled(free)
    .register();
    let add_rectangle = add(
        "canvas.add.rectangle",
        "Add a rectangle",
        ICON_RECTANGLE_HORIZONTAL,
        Tool::Rectangle,
    )
    .register();
    let add_line = add("canvas.add.line", "Add a line", ICON_SLASH, Tool::Line).register();
    let add_text = add("canvas.add.text", "Add text", ICON_TYPE, Tool::Text).register();
    let add_image = Action::new("canvas.add.image", "Add an image…", move || {
        state.open_image_picker(Some(
            state
                .take_context_position()
                .unwrap_or_else(|| state.viewport_center()),
        ))
    })
    .glyph(ICON_IMAGE)
    .shortcut(Chord::key(Key::Nine))
    .enabled(free)
    .register();
    let add_block = Action::new("canvas.add.block", "Add a block…", move || {
        state.open_block_picker(state.take_context_position())
    })
    .glyph(ICON_BRACES)
    .enabled(free)
    .register();
    let edit = Action::new("canvas.edit", "Open / edit", move || state.edit_selected())
        .glyph(ICON_PENCIL)
        .enabled(single)
        .register();
    let open = Action::new("canvas.open", "Open the block", move || {
        if let Some(block) = embedded_block(&state) {
            state.open_referenced_block(block);
        }
    })
    .glyph(ICON_EXTERNAL_LINK)
    .enabled(block)
    .register();
    let toggle_mode = Action::new(
        "canvas.toggle-mode",
        "Toggle preview / direct editor",
        move || state.toggle_selected_block_mode(),
    )
    .glyph(ICON_ARROW_LEFT_RIGHT)
    .enabled(block)
    .register();
    let front = layer(
        "canvas.bring-to-front",
        "Bring to front",
        ICON_LAYERS_ARROW_UP,
        CanvasLayerMove::BringToFront,
        Chord::ctrl(Key::BracketRight).shift(),
    )
    .register();
    let forward = layer(
        "canvas.forward",
        "Forward",
        ICON_ARROW_UP,
        CanvasLayerMove::ForwardOne,
        Chord::ctrl(Key::BracketRight),
    )
    .register();
    let backward = layer(
        "canvas.backward",
        "Backward",
        ICON_ARROW_DOWN,
        CanvasLayerMove::BackOne,
        Chord::ctrl(Key::BracketLeft),
    )
    .register();
    let back = layer(
        "canvas.send-to-back",
        "Send to back",
        ICON_SEND_TO_BACK,
        CanvasLayerMove::SendToBack,
        Chord::ctrl(Key::BracketLeft).shift(),
    )
    .register();
    let zoom_in = Action::new("canvas.zoom-in", "Zoom in", move || {
        state.editor().zoom(ZOOM_STEP)
    })
    .glyph(ICON_ZOOM_IN)
    .enabled(live)
    .register();
    let zoom_out = Action::new("canvas.zoom-out", "Zoom out", move || {
        state.editor().zoom(1.0 / ZOOM_STEP)
    })
    .glyph(ICON_ZOOM_OUT)
    .enabled(live)
    .register();
    let zoom_reset = Action::new("canvas.zoom-reset", "Zoom to 100%", move || {
        state
            .editor()
            .zoom(1.0 / scale.get_untracked().max(f32::EPSILON));
    })
    .enabled(live)
    .register();
    let fit_all = Action::new("canvas.fit-all", "Fit all", move || state.request_fit_all())
        .glyph(ICON_SCAN)
        .enabled(live)
        .register();
    let touch = TouchChoice::new();
    let ignore_touch = touch.action();
    let grid = grid_actions(state);
    CanvasActions {
        select,
        lasso,
        select_mode,
        frame,
        line,
        arrow,
        rectangle,
        ellipse,
        diamond,
        text,
        pen,
        eraser,
        tool_lock,
        bind_lines,
        add_rectangle,
        add_line,
        add_text,
        add_image,
        add_block,
        edit,
        open,
        toggle_mode,
        fit_selection,
        fit_frame,
        cut,
        copy,
        paste,
        copy_png,
        copy_svg,
        export_png,
        export_svg,
        exporter,
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
        ignore_touch,
        touch,
        grid,
        prototyping,
        vector,
        raster_profile,
        pixel,
        raster,
    }
}

pub(crate) fn embedded_block(state: &CanvasState) -> Option<Uuid> {
    let selected = state.selected_entities();
    let [entity] = selected.as_slice() else {
        return None;
    };
    entity.block.filter(|_| entity.shape.embeds_block())
}
