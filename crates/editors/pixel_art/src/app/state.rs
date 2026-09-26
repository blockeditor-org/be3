use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_editor_beui::be_block::PixelArtContent;
use block_editor_beui::be_block::pixel_art::Artwork;
use block_editor_beui::be_block::pixel_art::{PixelArtAnchor, PixelArtOperation, PixelColor};
use block_editor_beui::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_beui::{ContentProjection, Editor};

pub(crate) type ArtBlock = Rc<ContentProjection<PixelArtContent>>;

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

pub(crate) struct Tools {
    editor: Editor,
    block: ArtBlock,
    held: RefCell<Option<(u64, Rc<Artwork>)>>,
    pub(crate) tool: ReadSignal<PixelTool>,
    set_tool: WriteSignal<PixelTool>,
    previous_drawing_tool: Cell<PixelTool>,
    pub(crate) color: ReadSignal<PixelColor>,
    set_color: WriteSignal<PixelColor>,
    pub(crate) color_hex: ReadSignal<String>,
    set_color_hex: WriteSignal<String>,
    pub(crate) recent: ReadSignal<Vec<PixelColor>>,
    set_recent: WriteSignal<Vec<PixelColor>>,
    pub(crate) brush_size: ReadSignal<u16>,
    pub(crate) set_brush_size: WriteSignal<u16>,
    pub(crate) brush_shape: ReadSignal<BrushShape>,
    pub(crate) set_brush_shape: WriteSignal<BrushShape>,
    pub(crate) shapes_filled: ReadSignal<bool>,
    pub(crate) set_shapes_filled: WriteSignal<bool>,
    pub(crate) mirror_horizontal: ReadSignal<bool>,
    pub(crate) set_mirror_horizontal: WriteSignal<bool>,
    pub(crate) mirror_vertical: ReadSignal<bool>,
    pub(crate) set_mirror_vertical: WriteSignal<bool>,
    pub(crate) show_grid: ReadSignal<bool>,
    pub(crate) set_show_grid: WriteSignal<bool>,
    pub(crate) dialog: ReadSignal<Dialog>,
    set_dialog: WriteSignal<Dialog>,
    pub(crate) resize_width: ReadSignal<u16>,
    pub(crate) set_resize_width: WriteSignal<u16>,
    pub(crate) resize_height: ReadSignal<u16>,
    pub(crate) set_resize_height: WriteSignal<u16>,
    pub(crate) resize_anchor: ReadSignal<PixelArtAnchor>,
    pub(crate) set_resize_anchor: WriteSignal<PixelArtAnchor>,
    pub(crate) export_error: ReadSignal<Option<String>>,
    pub(crate) set_export_error: WriteSignal<Option<String>>,
    drawing: RefCell<Option<ActiveDrawing>>,
    drawn: ReadSignal<u64>,
    set_drawn: WriteSignal<u64>,
    constrained: ReadSignal<bool>,
    pub(crate) set_constrained: WriteSignal<bool>,
}

impl Tools {
    pub(crate) fn new(editor: &Editor, block: ArtBlock) -> Rc<Self> {
        let black = PixelColor::new(0, 0, 0, 255);
        let (tool, set_tool) = create_signal(PixelTool::Pencil);
        let (color, set_color) = create_signal(black);
        let (color_hex, set_color_hex) = create_signal(format_hex_color(black));
        let (recent, set_recent) = create_signal(vec![black]);
        let (brush_size, set_brush_size) = create_signal(1u16);
        let (brush_shape, set_brush_shape) = create_signal(BrushShape::Square);
        let (shapes_filled, set_shapes_filled) = create_signal(false);
        let (mirror_horizontal, set_mirror_horizontal) = create_signal(false);
        let (mirror_vertical, set_mirror_vertical) = create_signal(false);
        let (show_grid, set_show_grid) = create_signal(true);
        let (dialog, set_dialog) = create_signal(Dialog::None);
        let (resize_width, set_resize_width) = create_signal(32u16);
        let (resize_height, set_resize_height) = create_signal(32u16);
        let (resize_anchor, set_resize_anchor) = create_signal(PixelArtAnchor::Center);
        let (export_error, set_export_error) = create_signal(None::<String>);
        let (drawn, set_drawn) = create_signal(0);
        let (constrained, set_constrained) = create_signal(false);
        Rc::new(Self {
            editor: editor.clone(),
            block,
            held: RefCell::new(None),
            tool,
            set_tool,
            previous_drawing_tool: Cell::new(PixelTool::Pencil),
            color,
            set_color,
            color_hex,
            set_color_hex,
            recent,
            set_recent,
            brush_size,
            set_brush_size,
            brush_shape,
            set_brush_shape,
            shapes_filled,
            set_shapes_filled,
            mirror_horizontal,
            set_mirror_horizontal,
            mirror_vertical,
            set_mirror_vertical,
            show_grid,
            set_show_grid,
            dialog,
            set_dialog,
            resize_width,
            set_resize_width,
            resize_height,
            set_resize_height,
            resize_anchor,
            set_resize_anchor,
            export_error,
            set_export_error,
            drawing: RefCell::default(),
            drawn,
            set_drawn,
            constrained,
            set_constrained,
        })
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn block(&self) -> &ArtBlock {
        &self.block
    }

    pub(crate) fn artwork(&self) -> Option<Rc<Artwork>> {
        artwork_of(&self.block, &self.held)
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
        if let Some(edit) = self.block.read(|art| art.root().edit_for(&operation))
            && !edit.0.is_empty()
        {
            self.block.operate(edit);
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
            self.previous_drawing_tool.set(current);
        }
        if tool.is_drawing() {
            self.previous_drawing_tool.set(tool);
        }
        self.set_tool.set(tool);
    }

    pub(crate) fn previous_drawing_tool(&self) -> PixelTool {
        self.previous_drawing_tool.get()
    }

    pub(crate) fn set_active_color(&self, color: PixelColor, remember: bool) {
        self.set_color.set(color);
        self.set_color_hex.set(format_hex_color(color));
        if remember {
            self.remember_color(color);
        }
    }

    pub(crate) fn set_color_text(&self, text: String) {
        self.set_color_hex.set(text);
    }

    pub(crate) fn remember_color(&self, color: PixelColor) {
        self.set_recent.update(|recent| {
            recent.retain(|remembered| *remembered != color);
            recent.insert(0, color);
            recent.truncate(MAX_RECENT_COLORS);
        });
    }

    pub(crate) fn dialog(&self) -> Memo<Dialog> {
        let dialog = self.dialog.clone();
        create_memo(move || dialog.get())
    }

    pub(crate) fn open_dialog(&self, dialog: Dialog) {
        self.draw(|drawing| drawing.take());
        self.set_dialog.set(dialog);
    }

    pub(crate) fn close_dialog(&self) {
        self.set_dialog.set(Dialog::None);
    }

    pub(crate) fn busy(&self) -> bool {
        self.dialog.get() != Dialog::None
    }

    pub(crate) fn draw<R>(&self, change: impl FnOnce(&mut Option<ActiveDrawing>) -> R) -> R {
        let changed = change(&mut self.drawing.borrow_mut());
        self.set_drawn.update(|drawn| *drawn += 1);
        changed
    }

    pub(crate) fn drawing<R>(&self, read: impl FnOnce(Option<&ActiveDrawing>) -> R) -> R {
        self.drawn.get();
        read(self.drawing.borrow().as_ref())
    }
}

pub(crate) fn artwork_of(
    block: &ArtBlock,
    held: &RefCell<Option<(u64, Rc<Artwork>)>>,
) -> Option<Rc<Artwork>> {
    let revision = block.revision()?;
    if let Some((seen, art)) = &*held.borrow()
        && *seen == revision
    {
        return Some(Rc::clone(art));
    }
    let art = Rc::new(block.read(|content| content.root().artwork())?);
    held.replace(Some((revision, Rc::clone(&art))));
    Some(art)
}
