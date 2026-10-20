use std::io::{IsTerminal as _, Write as _};

pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

pub fn prompt(text: &str) -> Option<String> {
    eprint!("{text}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(
            line.chars()
                .filter(|character| !character.is_whitespace())
                .collect(),
        ),
    }
}

pub fn prompt_hidden(text: &str) -> Option<String> {
    let echo = Echo::off();
    let answer = prompt(text);
    drop(echo);
    eprintln!();
    answer
}

pub fn columns() -> usize {
    if let Some(columns) = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
    {
        return columns;
    }
    #[cfg(unix)]
    {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0
            && size.ws_col > 0
        {
            return usize::from(size.ws_col);
        }
    }
    80
}

#[cfg(unix)]
struct Echo(Option<libc::termios>);

#[cfg(unix)]
impl Echo {
    fn off() -> Echo {
        let mut settings: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut settings) } != 0 {
            return Echo(None);
        }
        let saved = settings;
        settings.c_lflag &= !libc::ECHO;
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &settings) };
        Echo(Some(saved))
    }
}

#[cfg(unix)]
impl Drop for Echo {
    fn drop(&mut self) {
        if let Some(saved) = &self.0 {
            unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, saved) };
        }
    }
}

#[cfg(windows)]
struct Echo(Option<u32>);

#[cfg(windows)]
impl Echo {
    fn off() -> Echo {
        use windows_sys::Win32::System::Console::{
            ENABLE_ECHO_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode,
        };
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return Echo(None);
            }
            SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT);
            Echo(Some(mode))
        }
    }
}

#[cfg(windows)]
impl Drop for Echo {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode};
        if let Some(mode) = self.0 {
            unsafe { SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), mode) };
        }
    }
}

#[cfg(not(any(unix, windows)))]
struct Echo;

#[cfg(not(any(unix, windows)))]
impl Echo {
    fn off() -> Echo {
        Echo
    }
}
