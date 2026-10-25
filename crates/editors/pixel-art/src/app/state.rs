use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_editor_beui::Fit;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{ContentProjection, Editor, Fitting};
use pixel_art_types::Artwork;
use pixel_art_types::PixelArtContent;
use pixel_art_types::{PixelArtAnchor, PixelArtOperation, PixelColor};

pub(crate) type ArtBlock = ContentProjection<PixelArtContent>;

use crate::color::format_hex_color;
use crate::drawing::{ActiveDrawing, Brush, BrushShape, PixelTool};

const MAX_RECENT_COLORS: usize = 12;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Dialog {
    #[default]
    None,
    Resize,
    Clear,
}

struct ToolsCells {
    block: ArtBlock,
    held: RefCell<Option<(u64, Rc<Artwork>)>>,
    previous_drawing_tool: Cell<PixelTool>,
    drawing: RefCell<Option<ActiveDrawing>>,
}

#[derive(Clone, Copy)]
pub(crate) struct Tools {
    cells: Stored<ToolsCells>,
    editor: Editor,
    pub(crate) fitting: Fitting,
    pub(crate) tool: Signal<PixelTool>,
    pub(crate) color: Signal<PixelColor>,
    pub(crate) color_hex: Signal<String>,
    pub(crate) recent: Signal<Vec<PixelColor>>,
    pub(crate) brush_size: Signal<u16>,
    pub(crate) brush_shape: Signal<BrushShape>,
    pub(crate) shapes_filled: Signal<bool>,
    pub(crate) mirror_horizontal: Signal<bool>,
    pub(crate) mirror_vertical: Signal<bool>,
    pub(crate) show_grid: Signal<bool>,
    pub(crate) dialog: Signal<Dialog>,
    pub(crate) resize_width: Signal<u16>,
    pub(crate) resize_height: Signal<u16>,
    pub(crate) resize_anchor: Signal<PixelArtAnchor>,
    pub(crate) export_error: Signal<Option<String>>,
    drawn: Signal<u64>,
    pub(crate) constrained: Signal<bool>,
}

impl Tools {
    pub(crate) fn new(editor: Editor, block: ArtBlock) -> Self {
        let black = PixelColor::new(0, 0, 0, 255);
        let tool = create_signal(PixelTool::Pencil);
        let color = create_signal(black);
        let color_hex = create_signal(format_hex_color(black));
        let recent = create_signal(vec![black]);
        let brush_size = create_signal(1u16);
        let brush_shape = create_signal(BrushShape::Square);
        let shapes_filled = create_signal(false);
        let mirror_horizontal = create_signal(false);
        let mirror_vertical = create_signal(false);
        let show_grid = create_signal(true);
        let dialog = create_signal(Dialog::None);
        let resize_width = create_signal(32u16);
        let resize_height = create_signal(32u16);
        let resize_anchor = create_signal(PixelArtAnchor::Center);
        let export_error = create_signal(None::<String>);
        let drawn = create_signal(0);
        let constrained = create_signal(false);
        let fitting = editor.keep_fitted(|| Some(Fit::World));
        Self {
            cells: store(ToolsCells {
                block,
                held: RefCell::new(None),
                previous_drawing_tool: Cell::new(PixelTool::Pencil),
                drawing: RefCell::default(),
            }),
            editor,
            fitting,
            tool,
            color,
            color_hex,
            recent,
            brush_size,
            brush_shape,
            shapes_filled,
            mirror_horizontal,
            mirror_vertical,
            show_grid,
            dialog,
            resize_width,
            resize_height,
            resize_anchor,
            export_error,
            drawn,
            constrained,
        }
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn block(&self) -> ArtBlock {
        self.cells.value().block
    }

    pub(crate) fn artwork(&self) -> Option<Rc<Artwork>> {
        self.cells
            .with(|cells| artwork_of(&cells.block, &cells.held))
    }

    pub(crate) fn pixel(&self, x: u16, y: u16) -> Option<PixelColor> {
        self.artwork()?.pixel(x, y)
    }

    pub(crate) fn editable(&self) -> bool {
        self.editor.editable().get_untracked()
    }

    pub(crate) fn operate(&self, operation: PixelArtOperation) {
        if !self.editable() {
            return;
        }
        if let Ok(edit) = self
            .cells
            .value()
            .block
            .read(|art| art.root().edit_for(&operation))
            && !edit.0.is_empty()
        {
            self.cells.value().block.operate(edit);
        }
    }

    pub(crate) fn brush(&self) -> Brush {
        Brush {
            size: self.brush_size.get(),
            shape: self.brush_shape.get(),
            filled: self.shapes_filled.get(),
            mirror_horizontal: self.mirror_horizontal.get(),
            mirror_vertical: self.mirror_vertical.get(),
            constrained: self.constrained.get(),
        }
    }

    pub(crate) fn select_tool(&self, tool: PixelTool) {
        self.draw(|drawing| drawing.take());
        let current = self.tool.get_untracked();
        if current.is_drawing() {
            self.cells.value().previous_drawing_tool.set(current);
        }
        if tool.is_drawing() {
            self.cells.value().previous_drawing_tool.set(tool);
        }
        self.tool.set(tool);
    }

    pub(crate) fn previous_drawing_tool(&self) -> PixelTool {
        self.cells.value().previous_drawing_tool.get()
    }

    pub(crate) fn set_active_color(&self, color: PixelColor, remember: bool) {
        self.color.set(color);
        self.color_hex.set(format_hex_color(color));
        if remember {
            self.remember_color(color);
        }
    }

    pub(crate) fn set_color_text(&self, text: String) {
        self.color_hex.set(text);
    }

    pub(crate) fn remember_color(&self, color: PixelColor) {
        self.recent.update(|recent| {
            recent.retain(|remembered| *remembered != color);
            recent.insert(0, color);
            recent.truncate(MAX_RECENT_COLORS);
        });
    }

    pub(crate) fn dialog(&self) -> Memo<Dialog> {
        let dialog = self.dialog;
        create_memo(move || dialog.get())
    }

    pub(crate) fn open_dialog(&self, dialog: Dialog) {
        self.draw(|drawing| drawing.take());
        self.dialog.set(dialog);
    }

    pub(crate) fn close_dialog(&self) {
        self.dialog.set(Dialog::None);
    }

    pub(crate) fn busy(&self) -> bool {
        self.dialog.get() != Dialog::None
    }

    pub(crate) fn draw<R>(&self, change: impl FnOnce(&mut Option<ActiveDrawing>) -> R) -> R {
        let changed = change(&mut self.cells.value().drawing.borrow_mut());
        self.drawn.update(|drawn| *drawn += 1);
        changed
    }

    pub(crate) fn drawing<R>(&self, read: impl FnOnce(Option<&ActiveDrawing>) -> R) -> R {
        self.drawn.get();
        read(self.cells.value().drawing.borrow().as_ref())
    }
}

pub(crate) fn artwork_of(
    block: &ArtBlock,
    held: &RefCell<Option<(u64, Rc<Artwork>)>>,
) -> Option<Rc<Artwork>> {
    let revision = block.revision().ok()?;
    if let Some((seen, art)) = &*held.borrow()
        && *seen == revision
    {
        return Some(Rc::clone(art));
    }
    let art = Rc::new(block.read(|content| content.root().artwork()).ok()?);
    held.replace(Some((revision, Rc::clone(&art))));
    Some(art)
}
