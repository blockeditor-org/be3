use std::io::{IsTerminal as _, Write as _};
#[cfg(not(unix))]
use std::io::Read as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
    ClearLine,
    DeleteWord,
    Interrupt,
    End,
    Escape,
    Char(char),
    Other,
}

pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

pub fn columns() -> usize {
    if let Some(columns) = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|columns| *columns > 0)
    {
        return columns;
    }
    #[cfg(unix)]
    {
        for descriptor in [libc::STDERR_FILENO, libc::STDOUT_FILENO, libc::STDIN_FILENO] {
            let mut size: libc::winsize = unsafe { std::mem::zeroed() };
            if unsafe { libc::ioctl(descriptor, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_col > 0 {
                return usize::from(size.ws_col);
            }
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo, GetStdHandle, STD_ERROR_HANDLE,
        };
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
        if unsafe { GetConsoleScreenBufferInfo(GetStdHandle(STD_ERROR_HANDLE), &mut info) } != 0 {
            let width = info.srWindow.Right - info.srWindow.Left + 1;
            if width > 0 {
                return width as usize;
            }
        }
    }
    80
}

pub fn colors() -> bool {
    std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && std::env::var("TERM").map_or(true, |term| term != "dumb")
}

pub fn write(text: &str) {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(text.as_bytes());
    let _ = stderr.flush();
}

pub struct Raw {
    #[cfg(unix)]
    saved: libc::termios,
    #[cfg(windows)]
    saved: (u32, u32),
}

impl Raw {
    pub fn enter() -> Option<Raw> {
        if !interactive() || !colors() {
            return None;
        }
        Raw::enter_platform()
    }

    #[cfg(unix)]
    fn enter_platform() -> Option<Raw> {
        let mut settings: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut settings) } != 0 {
            return None;
        }
        let saved = settings;
        settings.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
        settings.c_iflag &= !(libc::IXON | libc::ICRNL);
        settings.c_cc[libc::VMIN] = 1;
        settings.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &settings) } != 0 {
            return None;
        }
        Some(Raw { saved })
    }

    #[cfg(windows)]
    fn enter_platform() -> Option<Raw> {
        use windows_sys::Win32::System::Console::{
            ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_INPUT,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_ERROR_HANDLE,
            STD_INPUT_HANDLE, SetConsoleMode,
        };
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let output = GetStdHandle(STD_ERROR_HANDLE);
            let (mut input_mode, mut output_mode) = (0, 0);
            if GetConsoleMode(input, &mut input_mode) == 0 || GetConsoleMode(output, &mut output_mode) == 0 {
                return None;
            }
            let raw_input = (input_mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            if SetConsoleMode(input, raw_input) == 0
                || SetConsoleMode(output, output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0
            {
                SetConsoleMode(input, input_mode);
                return None;
            }
            Some(Raw {
                saved: (input_mode, output_mode),
            })
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn enter_platform() -> Option<Raw> {
        None
    }

    pub fn key(&self) -> Key {
        let Some(byte) = read_byte() else {
            return Key::End;
        };
        match byte {
            b'\r' | b'\n' => Key::Enter,
            0x7f | 0x08 => Key::Backspace,
            0x15 => Key::ClearLine,
            0x17 => Key::DeleteWord,
            0x03 => Key::Interrupt,
            0x04 => Key::End,
            0x10 => Key::Up,
            0x0e => Key::Down,
            0x1b => escape(),
            byte if byte < 0x20 => Key::Other,
            byte => utf8(byte).map_or(Key::Other, Key::Char),
        }
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.saved);
        }
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Console::{
                GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, SetConsoleMode,
            };
            SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), self.saved.0);
            SetConsoleMode(GetStdHandle(STD_ERROR_HANDLE), self.saved.1);
        }
    }
}

fn read_byte() -> Option<u8> {
    let mut byte = [0];
    #[cfg(unix)]
    {
        loop {
            let read = unsafe { libc::read(libc::STDIN_FILENO, byte.as_mut_ptr().cast(), 1) };
            if read == 1 {
                return Some(byte[0]);
            }
            if read < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return None;
        }
    }
    #[cfg(not(unix))]
    match std::io::stdin().lock().read(&mut byte) {
        Ok(1) => Some(byte[0]),
        _ => None,
    }
}

fn more_input_waiting() -> bool {
    #[cfg(unix)]
    {
        let mut descriptor = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe { libc::poll(&mut descriptor, 1, 30) > 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn escape() -> Key {
    if !more_input_waiting() {
        return Key::Escape;
    }
    match read_byte() {
        Some(b'[' | b'O') => {}
        _ => return Key::Other,
    }
    loop {
        match read_byte() {
            Some(b'A') => return Key::Up,
            Some(b'B') => return Key::Down,
            Some(b'C') => return Key::Right,
            Some(b'D') => return Key::Left,
            Some(b'0'..=b'9' | b';') => continue,
            _ => return Key::Other,
        }
    }
}

fn utf8(first: u8) -> Option<char> {
    let length = match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let mut bytes = vec![first];
    for _ in 1..length {
        bytes.push(read_byte()?);
    }
    std::str::from_utf8(&bytes).ok()?.chars().next()
}
