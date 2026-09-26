use std::ffi::c_int;
use std::fmt;
use std::ptr;

use crate::sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    OutOfMemory,
    InvalidValue,
    OutOfSpace,
    NoValue,
    Unknown(c_int),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory => formatter.write_str("out of memory"),
            Self::InvalidValue => formatter.write_str("invalid value"),
            Self::OutOfSpace => formatter.write_str("out of space"),
            Self::NoValue => formatter.write_str("no value"),
            Self::Unknown(code) => write!(formatter, "unknown error {code}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn check(result: sys::Result) -> Result<(), Error> {
    match result {
        sys::SUCCESS => Ok(()),
        sys::OUT_OF_MEMORY => Err(Error::OutOfMemory),
        sys::INVALID_VALUE => Err(Error::InvalidValue),
        sys::OUT_OF_SPACE => Err(Error::OutOfSpace),
        sys::NO_VALUE => Err(Error::NoValue),
        code => Err(Error::Unknown(code)),
    }
}

type Reply = Box<dyn FnMut(&[u8])>;

pub struct Terminal {
    handle: sys::Terminal,
    reply: Option<Box<Reply>>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16, max_scrollback: usize) -> Result<Self, Error> {
        let mut handle: sys::Terminal = ptr::null_mut();
        check(unsafe {
            sys::ghostty_terminal_new(ptr::null(), &mut handle, cols.max(1), rows.max(1))
        })?;
        let terminal = Self {
            handle,
            reply: None,
        };
        check(unsafe {
            sys::ghostty_terminal_set(
                terminal.handle,
                sys::TERMINAL_OPT_SCROLLBACK_MAX_LINES,
                ptr::from_ref(&max_scrollback).cast(),
            )
        })?;
        Ok(terminal)
    }

    pub fn on_reply(&mut self, reply: impl FnMut(&[u8]) + 'static) -> Result<(), Error> {
        let mut reply: Box<Reply> = Box::new(Box::new(reply));
        let userdata: *mut Reply = &mut *reply;
        check(unsafe {
            sys::ghostty_terminal_set(self.handle, sys::TERMINAL_OPT_USERDATA, userdata.cast())
        })?;
        let callback: sys::WritePtyFn = forward_reply;
        check(unsafe {
            sys::ghostty_terminal_set(
                self.handle,
                sys::TERMINAL_OPT_WRITE_PTY,
                callback as *const std::ffi::c_void,
            )
        })?;
        self.reply = Some(reply);
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        unsafe { sys::ghostty_terminal_vt_write(self.handle, data.as_ptr(), data.len()) }
    }

    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width: u32,
        cell_height: u32,
    ) -> Result<(), Error> {
        check(unsafe {
            sys::ghostty_terminal_resize(
                self.handle,
                cols.max(1),
                rows.max(1),
                cell_width,
                cell_height,
            )
        })
    }

    pub fn reset(&mut self) {
        unsafe { sys::ghostty_terminal_reset(self.handle) }
    }

    pub fn scroll_by(&mut self, rows: isize) {
        self.scroll(sys::ScrollViewport {
            tag: sys::SCROLL_VIEWPORT_DELTA,
            value: sys::ScrollViewportValue { delta: rows },
        });
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll(sys::ScrollViewport {
            tag: sys::SCROLL_VIEWPORT_TOP,
            value: sys::ScrollViewportValue { padding: [0, 0] },
        });
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll(sys::ScrollViewport {
            tag: sys::SCROLL_VIEWPORT_BOTTOM,
            value: sys::ScrollViewportValue { padding: [0, 0] },
        });
    }

    fn scroll(&mut self, behavior: sys::ScrollViewport) {
        unsafe { sys::ghostty_terminal_scroll_viewport(self.handle, behavior) }
    }

    pub(crate) fn handle(&self) -> sys::Terminal {
        self.handle
    }
}

unsafe extern "C" fn forward_reply(
    _terminal: sys::Terminal,
    userdata: *mut std::ffi::c_void,
    data: *const u8,
    len: usize,
) {
    if userdata.is_null() || data.is_null() {
        return;
    }
    let reply = unsafe { &mut *userdata.cast::<Reply>() };
    reply(unsafe { std::slice::from_raw_parts(data, len) });
}

impl Drop for Terminal {
    fn drop(&mut self) {
        unsafe { sys::ghostty_terminal_free(self.handle) }
    }
}
