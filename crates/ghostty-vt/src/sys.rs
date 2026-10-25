use std::ffi::{c_int, c_void};

pub type Result = c_int;

pub const SUCCESS: Result = 0;
pub const OUT_OF_MEMORY: Result = -1;
pub const INVALID_VALUE: Result = -2;
pub const OUT_OF_SPACE: Result = -3;
pub const NO_VALUE: Result = -4;

pub type Terminal = *mut c_void;
pub type RenderState = *mut c_void;
pub type RowIterator = *mut c_void;
pub type RowCells = *mut c_void;
pub type TrackedGridRef = *mut c_void;
pub type KeyEncoder = *mut c_void;
pub type KeyEvent = *mut c_void;
pub type MouseEncoder = *mut c_void;
pub type MouseEvent = *mut c_void;
pub type CellValue = u64;
pub type RowValue = u64;

pub const TERMINAL_DATA_COLS: c_int = 1;
pub const TERMINAL_DATA_ROWS: c_int = 2;
pub const TERMINAL_DATA_CURSOR_X: c_int = 3;
pub const TERMINAL_DATA_CURSOR_Y: c_int = 4;
pub const TERMINAL_DATA_ACTIVE_SCREEN: c_int = 6;
pub const TERMINAL_DATA_CURSOR_VISIBLE: c_int = 7;
pub const TERMINAL_DATA_MOUSE_TRACKING: c_int = 11;
pub const TERMINAL_DATA_TITLE: c_int = 12;
pub const TERMINAL_DATA_SCROLLBACK_ROWS: c_int = 15;
pub const TERMINAL_DATA_COLOR_FOREGROUND: c_int = 18;
pub const TERMINAL_DATA_COLOR_BACKGROUND: c_int = 19;
pub const TERMINAL_DATA_COLOR_PALETTE: c_int = 21;
pub const TERMINAL_DATA_MODE: c_int = 37;

pub const TERMINAL_SCREEN_ALTERNATE: c_int = 1;

pub const POINT_TAG_ACTIVE: c_int = 0;
pub const POINT_TAG_HISTORY: c_int = 3;

pub const CELL_DATA_CONTENT_TAG: c_int = 2;
pub const CELL_DATA_WIDE: c_int = 3;
pub const CELL_DATA_HAS_STYLING: c_int = 5;
pub const CELL_DATA_COLOR_PALETTE: c_int = 10;
pub const CELL_DATA_COLOR_RGB: c_int = 11;

pub const CELL_CONTENT_BG_COLOR_PALETTE: c_int = 2;
pub const CELL_CONTENT_BG_COLOR_RGB: c_int = 3;

pub const CELL_WIDE_SPACER_TAIL: c_int = 2;
pub const CELL_WIDE_SPACER_HEAD: c_int = 3;

pub const ROW_DATA_WRAP: c_int = 1;

pub const RENDER_STATE_DATA_CURSOR_VISUAL_STYLE: c_int = 10;
pub const RENDER_STATE_DATA_CURSOR_BLINKING: c_int = 12;
pub const RENDER_STATE_ROW_DATA_RAW: c_int = 2;
pub const CELLS_DATA_RAW: c_int = 1;

pub const STYLE_COLOR_PALETTE: c_int = 1;
pub const STYLE_COLOR_RGB: c_int = 2;

pub const KEY_ACTION_RELEASE: c_int = 0;
pub const KEY_ACTION_PRESS: c_int = 1;

pub const MODS_SHIFT: u16 = 1 << 0;
pub const MODS_CTRL: u16 = 1 << 1;
pub const MODS_ALT: u16 = 1 << 2;
pub const MODS_SUPER: u16 = 1 << 3;

pub const MOUSE_ACTION_PRESS: c_int = 0;
pub const MOUSE_BUTTON_FOUR: c_int = 4;
pub const MOUSE_BUTTON_FIVE: c_int = 5;
pub const MOUSE_ENCODER_OPT_SIZE: c_int = 2;

pub const CLIPBOARD_LOCATION_STANDARD: c_int = 0;
pub const PASTE_SOURCE_CLIPBOARD: c_int = 0;

pub const RENDER_STATE_DATA_ROW_ITERATOR: c_int = 4;
pub const RENDER_STATE_DATA_CURSOR_VISIBLE: c_int = 11;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE: c_int = 14;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_X: c_int = 15;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_Y: c_int = 16;
pub const RENDER_STATE_DATA_COLORS: c_int = 19;

pub const TERMINAL_OPT_USERDATA: c_int = 0;
pub const TERMINAL_OPT_WRITE_PTY: c_int = 1;
pub const TERMINAL_OPT_SCROLLBACK_MAX_LINES: c_int = 28;

pub type WritePtyFn =
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut c_void, data: *const u8, len: usize);

pub const RENDER_STATE_OPTION_DIRTY: c_int = 0;
pub const RENDER_STATE_DIRTY_FALSE: c_int = 0;

pub const ROW_DATA_CELLS: c_int = 3;
pub const ROW_OPTION_DIRTY: c_int = 0;

pub const CELLS_DATA_STYLE: c_int = 2;
pub const CELLS_DATA_GRAPHEMES_LEN: c_int = 3;
pub const CELLS_DATA_BG_COLOR: c_int = 5;
pub const CELLS_DATA_FG_COLOR: c_int = 6;
pub const CELLS_DATA_HAS_STYLING: c_int = 8;
pub const CELLS_DATA_GRAPHEMES_UTF8: c_int = 9;

pub const SCROLL_VIEWPORT_TOP: c_int = 0;
pub const SCROLL_VIEWPORT_BOTTOM: c_int = 1;
pub const SCROLL_VIEWPORT_DELTA: c_int = 2;

pub const STYLE_COLOR_NONE: c_int = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union ScrollViewportValue {
    pub delta: isize,
    pub row: usize,
    pub padding: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ScrollViewport {
    pub tag: c_int,
    pub value: ScrollViewportValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RenderStateColors {
    pub size: usize,
    pub background: ColorRgb,
    pub foreground: ColorRgb,
    pub cursor: ColorRgb,
    pub cursor_has_value: bool,
    pub palette: [ColorRgb; 256],
}

impl Default for RenderStateColors {
    fn default() -> Self {
        Self {
            size: size_of::<Self>(),
            background: ColorRgb::default(),
            foreground: ColorRgb::default(),
            cursor: ColorRgb::default(),
            cursor_has_value: false,
            palette: [ColorRgb::default(); 256],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union StyleColorValue {
    pub palette: u8,
    pub rgb: ColorRgb,
    pub padding: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StyleColor {
    pub tag: c_int,
    pub value: StyleColorValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Style {
    pub size: usize,
    pub fg_color: StyleColor,
    pub bg_color: StyleColor,
    pub underline_color: StyleColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: c_int,
}

impl Default for Style {
    fn default() -> Self {
        let none = StyleColor {
            tag: STYLE_COLOR_NONE,
            value: StyleColorValue { padding: 0 },
        };
        Self {
            size: size_of::<Self>(),
            fg_color: none,
            bg_color: none,
            underline_color: none,
            bold: false,
            italic: false,
            faint: false,
            blink: false,
            inverse: false,
            invisible: false,
            strikethrough: false,
            overline: false,
            underline: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GridRef {
    pub size: usize,
    pub node: *mut c_void,
    pub x: u16,
    pub y: u16,
}

impl Default for GridRef {
    fn default() -> Self {
        Self {
            size: size_of::<Self>(),
            node: std::ptr::null_mut(),
            x: 0,
            y: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PointCoordinate {
    pub x: u16,
    pub y: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union PointValue {
    pub coordinate: PointCoordinate,
    pub padding: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub tag: c_int,
    pub value: PointValue,
}

impl Point {
    pub fn new(tag: c_int, x: u16, y: u32) -> Self {
        let mut value = PointValue { padding: [0, 0] };
        value.coordinate = PointCoordinate { x, y };
        Self { tag, value }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalModeConfig {
    pub mode: u16,
    pub value: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyString {
    pub ptr: *const u8,
    pub len: usize,
}

pub type WriterFn =
    unsafe extern "C" fn(userdata: *mut c_void, data: *const u8, len: usize) -> bool;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Writer {
    pub write: Option<WriterFn>,
    pub userdata: *mut c_void,
}

pub type MimeReaderFn =
    unsafe extern "C" fn(userdata: *mut c_void, mime: GhosttyString, writer: Writer) -> bool;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MimeReader {
    pub read: Option<MimeReaderFn>,
    pub userdata: *mut c_void,
}

#[repr(C)]
pub struct Paste {
    pub size: usize,
    pub location: c_int,
    pub source: c_int,
    pub mimes: *const GhosttyString,
    pub mimes_len: usize,
    pub reader: MimeReader,
    pub allow_unsafe: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MousePosition {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MouseEncoderSize {
    pub size: usize,
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
    pub padding_left: u32,
}

#[repr(C)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub cap: usize,
    pub len: usize,
}

// unsafe(ffi)
unsafe extern "C" {
    pub fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut Terminal,
        cols: u16,
        rows: u16,
    ) -> Result;
    pub fn ghostty_terminal_free(terminal: Terminal);
    pub fn ghostty_terminal_set(terminal: Terminal, option: c_int, value: *const c_void) -> Result;
    pub fn ghostty_terminal_reset(terminal: Terminal);
    pub fn ghostty_terminal_resize(
        terminal: Terminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result;
    pub fn ghostty_terminal_vt_write(terminal: Terminal, data: *const u8, len: usize);
    pub fn ghostty_terminal_scroll_viewport(terminal: Terminal, behavior: ScrollViewport);

    pub fn ghostty_render_state_new(allocator: *const c_void, state: *mut RenderState) -> Result;
    pub fn ghostty_render_state_free(state: RenderState);
    pub fn ghostty_render_state_update(state: RenderState, terminal: Terminal) -> Result;
    pub fn ghostty_render_state_get(state: RenderState, data: c_int, out: *mut c_void) -> Result;
    pub fn ghostty_render_state_set(
        state: RenderState,
        option: c_int,
        value: *const c_void,
    ) -> Result;

    pub fn ghostty_render_state_row_iterator_new(
        allocator: *const c_void,
        out_iterator: *mut RowIterator,
    ) -> Result;
    pub fn ghostty_render_state_row_iterator_free(iterator: RowIterator);
    pub fn ghostty_render_state_row_iterator_next(iterator: RowIterator) -> bool;
    pub fn ghostty_render_state_row_get(
        iterator: RowIterator,
        data: c_int,
        out: *mut c_void,
    ) -> Result;
    pub fn ghostty_render_state_row_set(
        iterator: RowIterator,
        option: c_int,
        value: *const c_void,
    ) -> Result;

    pub fn ghostty_render_state_row_cells_new(
        allocator: *const c_void,
        out_cells: *mut RowCells,
    ) -> Result;
    pub fn ghostty_render_state_row_cells_free(cells: RowCells);
    pub fn ghostty_render_state_row_cells_next(cells: RowCells) -> bool;
    pub fn ghostty_render_state_row_cells_get(
        cells: RowCells,
        data: c_int,
        out: *mut c_void,
    ) -> Result;

    pub fn ghostty_terminal_get(terminal: Terminal, data: c_int, out: *mut c_void) -> Result;
    pub fn ghostty_terminal_grid_ref(terminal: Terminal, point: Point, out: *mut GridRef)
    -> Result;
    pub fn ghostty_terminal_grid_ref_track(
        terminal: Terminal,
        point: Point,
        out: *mut TrackedGridRef,
    ) -> Result;
    pub fn ghostty_terminal_paste(
        terminal: Terminal,
        paste: *const Paste,
        out_written: *mut bool,
    ) -> Result;

    pub fn ghostty_tracked_grid_ref_free(reference: TrackedGridRef);
    pub fn ghostty_tracked_grid_ref_point(
        reference: TrackedGridRef,
        tag: c_int,
        out: *mut PointCoordinate,
    ) -> Result;
    pub fn ghostty_tracked_grid_ref_set(
        reference: TrackedGridRef,
        terminal: Terminal,
        point: Point,
    ) -> Result;

    pub fn ghostty_grid_ref_cell(reference: *const GridRef, out: *mut CellValue) -> Result;
    pub fn ghostty_grid_ref_row(reference: *const GridRef, out: *mut RowValue) -> Result;
    pub fn ghostty_grid_ref_graphemes(
        reference: *const GridRef,
        buffer: *mut u32,
        length: usize,
        out_length: *mut usize,
    ) -> Result;
    pub fn ghostty_grid_ref_style(reference: *const GridRef, out: *mut Style) -> Result;

    pub fn ghostty_cell_get(cell: CellValue, data: c_int, out: *mut c_void) -> Result;
    pub fn ghostty_row_get(row: RowValue, data: c_int, out: *mut c_void) -> Result;

    pub fn ghostty_key_encoder_new(allocator: *const c_void, encoder: *mut KeyEncoder) -> Result;
    pub fn ghostty_key_encoder_free(encoder: KeyEncoder);
    pub fn ghostty_key_encoder_setopt_from_terminal(encoder: KeyEncoder, terminal: Terminal);
    pub fn ghostty_key_encoder_encode(
        encoder: KeyEncoder,
        event: KeyEvent,
        out: *mut std::ffi::c_char,
        capacity: usize,
        out_length: *mut usize,
    ) -> Result;

    pub fn ghostty_key_event_new(allocator: *const c_void, event: *mut KeyEvent) -> Result;
    pub fn ghostty_key_event_free(event: KeyEvent);
    pub fn ghostty_key_event_set_action(event: KeyEvent, action: c_int);
    pub fn ghostty_key_event_set_key(event: KeyEvent, key: c_int);
    pub fn ghostty_key_event_set_mods(event: KeyEvent, mods: u16);
    pub fn ghostty_key_event_set_consumed_mods(event: KeyEvent, mods: u16);
    pub fn ghostty_key_event_set_utf8(
        event: KeyEvent,
        utf8: *const std::ffi::c_char,
        length: usize,
    );
    pub fn ghostty_key_event_set_unshifted_codepoint(event: KeyEvent, codepoint: u32);

    pub fn ghostty_mouse_encoder_new(
        allocator: *const c_void,
        encoder: *mut MouseEncoder,
    ) -> Result;
    pub fn ghostty_mouse_encoder_free(encoder: MouseEncoder);
    pub fn ghostty_mouse_encoder_setopt(encoder: MouseEncoder, option: c_int, value: *const c_void);
    pub fn ghostty_mouse_encoder_setopt_from_terminal(encoder: MouseEncoder, terminal: Terminal);
    pub fn ghostty_mouse_encoder_encode(
        encoder: MouseEncoder,
        event: MouseEvent,
        out: *mut std::ffi::c_char,
        capacity: usize,
        out_length: *mut usize,
    ) -> Result;

    pub fn ghostty_mouse_event_new(allocator: *const c_void, event: *mut MouseEvent) -> Result;
    pub fn ghostty_mouse_event_free(event: MouseEvent);
    pub fn ghostty_mouse_event_set_action(event: MouseEvent, action: c_int);
    pub fn ghostty_mouse_event_set_button(event: MouseEvent, button: c_int);
    pub fn ghostty_mouse_event_set_mods(event: MouseEvent, mods: u16);
    pub fn ghostty_mouse_event_set_position(event: MouseEvent, position: MousePosition);

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ghostty_type_json() -> *const std::ffi::c_char;
}
