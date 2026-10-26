use std::ffi::{c_int, c_void};
use std::ptr;

use crate::render::{Cell, Rgb, Row, is_spacer, row_flag};
use crate::sys;
use crate::terminal::{Error, Terminal, check};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveScreen {
    #[default]
    Primary,
    Alternate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode(u16);

impl Mode {
    pub const CURSOR_KEYS: Self = Self::private(1);
    pub const CURSOR_BLINKING: Self = Self::private(12);
    pub const ALTERNATE_SCROLL: Self = Self::private(1007);
    pub const BRACKETED_PASTE: Self = Self::private(2004);

    pub const fn private(value: u16) -> Self {
        Self(value & 0x7fff)
    }

    pub const fn ansi(value: u16) -> Self {
        Self((value & 0x7fff) | 0x8000)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    pub foreground: Rgb,
    pub background: Rgb,
    pub palette: [Rgb; 256],
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            foreground: Rgb::default(),
            background: Rgb::default(),
            palette: [Rgb::default(); 256],
        }
    }
}

// unsafe(ffi)
impl Terminal {
    fn get<T: Default>(&self, data: c_int) -> Option<T> {
        let mut value = T::default();
        let result = unsafe {
            sys::ghostty_terminal_get(self.handle(), data, ptr::from_mut(&mut value).cast())
        };
        (result == sys::SUCCESS).then_some(value)
    }

    pub fn size(&self) -> (u16, u16) {
        (
            self.get(sys::TERMINAL_DATA_COLS).unwrap_or(1),
            self.get(sys::TERMINAL_DATA_ROWS).unwrap_or(1),
        )
    }

    pub fn active_screen(&self) -> ActiveScreen {
        match self.get::<c_int>(sys::TERMINAL_DATA_ACTIVE_SCREEN) {
            Some(sys::TERMINAL_SCREEN_ALTERNATE) => ActiveScreen::Alternate,
            _ => ActiveScreen::Primary,
        }
    }

    pub fn scrollback_rows(&self) -> usize {
        self.get(sys::TERMINAL_DATA_SCROLLBACK_ROWS).unwrap_or(0)
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        (
            self.get(sys::TERMINAL_DATA_CURSOR_X).unwrap_or(0),
            self.get(sys::TERMINAL_DATA_CURSOR_Y).unwrap_or(0),
        )
    }

    pub fn cursor_visible(&self) -> bool {
        self.get(sys::TERMINAL_DATA_CURSOR_VISIBLE).unwrap_or(true)
    }

    pub fn mouse_tracking(&self) -> bool {
        self.get(sys::TERMINAL_DATA_MOUSE_TRACKING).unwrap_or(false)
    }

    pub fn mode(&self, mode: Mode) -> bool {
        let mut config = sys::TerminalModeConfig {
            mode: mode.0,
            value: false,
        };
        let result = unsafe {
            sys::ghostty_terminal_get(
                self.handle(),
                sys::TERMINAL_DATA_MODE,
                ptr::from_mut(&mut config).cast(),
            )
        };
        result == sys::SUCCESS && config.value
    }

    pub fn title(&self) -> String {
        let mut title = sys::GhosttyString {
            ptr: ptr::null(),
            len: 0,
        };
        let result = unsafe {
            sys::ghostty_terminal_get(
                self.handle(),
                sys::TERMINAL_DATA_TITLE,
                ptr::from_mut(&mut title).cast(),
            )
        };
        if result != sys::SUCCESS || title.ptr.is_null() || title.len == 0 {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(title.ptr, title.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    pub fn colors(&self) -> Colors {
        let mut colors = Colors {
            foreground: self
                .get(sys::TERMINAL_DATA_COLOR_FOREGROUND)
                .unwrap_or_default(),
            background: self
                .get(sys::TERMINAL_DATA_COLOR_BACKGROUND)
                .unwrap_or_default(),
            ..Colors::default()
        };
        let _ = unsafe {
            sys::ghostty_terminal_get(
                self.handle(),
                sys::TERMINAL_DATA_COLOR_PALETTE,
                colors.palette.as_mut_ptr().cast(),
            )
        };
        colors
    }

    pub fn active_row(&self, y: u16, colors: &Colors) -> Option<Row> {
        self.row(
            sys::Point::new(sys::POINT_TAG_ACTIVE, 0, u32::from(y)),
            colors,
        )
    }

    pub fn history_row(&self, y: usize, colors: &Colors) -> Option<Row> {
        let y = u32::try_from(y).ok()?;
        self.row(sys::Point::new(sys::POINT_TAG_HISTORY, 0, y), colors)
    }

    fn row(&self, point: sys::Point, colors: &Colors) -> Option<Row> {
        let mut reference = sys::GridRef::default();
        check(unsafe { sys::ghostty_terminal_grid_ref(self.handle(), point, &mut reference) })
            .ok()?;
        let mut raw: sys::RowValue = 0;
        let wrapped = unsafe { sys::ghostty_grid_ref_row(&reference, &mut raw) } == sys::SUCCESS
            && row_flag(raw, sys::ROW_DATA_WRAP);
        let (cols, _) = self.size();
        let mut cells = Vec::with_capacity(usize::from(cols));
        for x in 0..cols {
            reference.x = x;
            let mut cell = Cell::default();
            read_grid_cell(&reference, colors, &mut cell);
            cells.push(cell);
        }
        Some(Row { cells, wrapped })
    }

    pub fn paste(&mut self, text: &str) -> Result<(), Error> {
        let mime = b"text/plain";
        let mimes = [sys::GhosttyString {
            ptr: mime.as_ptr(),
            len: mime.len(),
        }];
        let mut source = text.as_bytes();
        let paste = sys::Paste {
            size: size_of::<sys::Paste>(),
            location: sys::CLIPBOARD_LOCATION_STANDARD,
            source: sys::PASTE_SOURCE_CLIPBOARD,
            mimes: mimes.as_ptr(),
            mimes_len: mimes.len(),
            reader: sys::MimeReader {
                read: Some(read_paste),
                userdata: ptr::from_mut(&mut source).cast(),
            },
            allow_unsafe: true,
        };
        let mut written = false;
        check(unsafe { sys::ghostty_terminal_paste(self.handle(), &paste, &mut written) })
    }
}

// unsafe(ffi)
unsafe extern "C" fn read_paste(
    userdata: *mut c_void,
    _mime: sys::GhosttyString,
    writer: sys::Writer,
) -> bool {
    if userdata.is_null() {
        return false;
    }
    let source = unsafe { &*userdata.cast::<&[u8]>() };
    match writer.write {
        Some(write) => unsafe { write(writer.userdata, source.as_ptr(), source.len()) },
        None => false,
    }
}

// unsafe(ffi)
fn read_grid_cell(reference: &sys::GridRef, colors: &Colors, cell: &mut Cell) {
    cell.reset(colors.foreground);
    let mut raw: sys::CellValue = 0;
    if unsafe { sys::ghostty_grid_ref_cell(reference, &mut raw) } != sys::SUCCESS {
        return;
    }
    if is_spacer(raw) {
        cell.spacer = true;
        return;
    }

    let mut codepoints = [0u32; 16];
    let mut length = 0usize;
    let result = unsafe {
        sys::ghostty_grid_ref_graphemes(
            reference,
            codepoints.as_mut_ptr(),
            codepoints.len(),
            &mut length,
        )
    };
    if result == sys::SUCCESS {
        cell.text.extend(
            codepoints[..length.min(codepoints.len())]
                .iter()
                .filter_map(|codepoint| char::from_u32(*codepoint)),
        );
    }

    let mut tag: c_int = 0;
    if cell_get(
        raw,
        sys::CELL_DATA_CONTENT_TAG,
        ptr::from_mut(&mut tag).cast(),
    ) {
        if tag == sys::CELL_CONTENT_BG_COLOR_PALETTE {
            let mut index: u8 = 0;
            if cell_get(
                raw,
                sys::CELL_DATA_COLOR_PALETTE,
                ptr::from_mut(&mut index).cast(),
            ) {
                cell.background = Some(colors.palette[usize::from(index)]);
            }
        } else if tag == sys::CELL_CONTENT_BG_COLOR_RGB {
            let mut rgb = Rgb::default();
            if cell_get(
                raw,
                sys::CELL_DATA_COLOR_RGB,
                ptr::from_mut(&mut rgb).cast(),
            ) {
                cell.background = Some(rgb);
            }
        }
    }

    let mut styled = false;
    if !cell_get(
        raw,
        sys::CELL_DATA_HAS_STYLING,
        ptr::from_mut(&mut styled).cast(),
    ) || !styled
    {
        return;
    }
    let mut style = sys::Style::default();
    if unsafe { sys::ghostty_grid_ref_style(reference, &mut style) } != sys::SUCCESS {
        return;
    }
    if let Some(foreground) = resolve(&style.fg_color, colors) {
        cell.foreground = foreground;
    }
    if let Some(background) = resolve(&style.bg_color, colors) {
        cell.background = Some(background);
    }
    cell.style(&style, colors.background);
}

// unsafe(ffi)
fn resolve(color: &sys::StyleColor, colors: &Colors) -> Option<Rgb> {
    match color.tag {
        sys::STYLE_COLOR_PALETTE => {
            Some(colors.palette[usize::from(unsafe { color.value.palette })])
        }
        sys::STYLE_COLOR_RGB => Some(unsafe { color.value.rgb }),
        _ => None,
    }
}

// unsafe(ffi)
fn cell_get(cell: sys::CellValue, data: c_int, out: *mut c_void) -> bool {
    unsafe { sys::ghostty_cell_get(cell, data, out) == sys::SUCCESS }
}

pub enum AnchorPlace {
    Active(u32),
    History(usize),
    Lost,
}

pub struct Anchor {
    handle: sys::TrackedGridRef,
}

// unsafe(ffi)
impl Anchor {
    pub fn new(terminal: &Terminal) -> Result<Self, Error> {
        let mut handle: sys::TrackedGridRef = ptr::null_mut();
        check(unsafe {
            sys::ghostty_terminal_grid_ref_track(
                terminal.handle(),
                sys::Point::new(sys::POINT_TAG_ACTIVE, 0, 0),
                &mut handle,
            )
        })?;
        Ok(Self { handle })
    }

    pub fn reset(&mut self, terminal: &Terminal) -> Result<(), Error> {
        check(unsafe {
            sys::ghostty_tracked_grid_ref_set(
                self.handle,
                terminal.handle(),
                sys::Point::new(sys::POINT_TAG_ACTIVE, 0, 0),
            )
        })
    }

    pub fn place(&self) -> AnchorPlace {
        let mut point = sys::PointCoordinate::default();
        if unsafe {
            sys::ghostty_tracked_grid_ref_point(self.handle, sys::POINT_TAG_ACTIVE, &mut point)
        } == sys::SUCCESS
        {
            return AnchorPlace::Active(point.y);
        }
        if unsafe {
            sys::ghostty_tracked_grid_ref_point(self.handle, sys::POINT_TAG_HISTORY, &mut point)
        } == sys::SUCCESS
        {
            return AnchorPlace::History(point.y as usize);
        }
        AnchorPlace::Lost
    }
}

// unsafe(ffi)
impl Drop for Anchor {
    fn drop(&mut self) {
        unsafe { sys::ghostty_tracked_grid_ref_free(self.handle) }
    }
}
