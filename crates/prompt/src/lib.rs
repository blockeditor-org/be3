use std::io::Write as _;
use std::path::{Path, PathBuf};

mod terminal;

pub use terminal::{columns, interactive};
use terminal::{Key, Raw, colors, write};

pub struct Choice<'a> {
    pub label: &'a str,
    pub detail: &'a str,
}

pub fn choose(question: &str, choices: &[Choice], default: usize) -> Option<usize> {
    if !interactive() || choices.is_empty() {
        return None;
    }
    let Some(raw) = Raw::enter() else {
        return choose_plainly(question, choices);
    };
    let width = choices.iter().map(|choice| choice.label.chars().count()).max().unwrap_or(0);
    let mut selected = default.min(choices.len() - 1);
    let mut screen = Screen::default();
    write("\x1b[?25l");
    let chosen = loop {
        let mut lines = vec![format!("{} {}", style("36;1", "?"), style("1", question))];
        for (index, choice) in choices.iter().enumerate() {
            let label = format!("{:width$}", choice.label);
            lines.push(if index == selected {
                format!("{} {}  {}", style("36;1", ">"), style("36;1", &label), choice.detail)
            } else {
                format!("  {}  {}", label, style("2", choice.detail))
            });
        }
        lines.push(style("2", "  up and down to move, enter to choose"));
        screen.draw(&lines);
        match raw.key() {
            Key::Up | Key::Char('k') => selected = (selected + choices.len() - 1) % choices.len(),
            Key::Down | Key::Char('j') | Key::Char('\t') => selected = (selected + 1) % choices.len(),
            Key::Char(digit @ '1'..='9') => {
                let index = digit as usize - '1' as usize;
                if index < choices.len() {
                    selected = index;
                }
            }
            Key::Enter => break Some(selected),
            Key::Interrupt => interrupted(raw, &mut screen),
            Key::End | Key::Escape => break None,
            _ => {}
        }
    };
    let answer = chosen.map_or_else(|| style("2", "skipped"), |index| style("36", choices[index].label));
    screen.draw(&[format!("{} {} {answer}", style("36;1", "?"), style("1", question))]);
    write("\n\x1b[?25h");
    drop(raw);
    chosen
}

pub fn text(question: &str, placeholder: &str, check: impl Fn(&str) -> Result<String, String>) -> Option<String> {
    line(question, placeholder, false, check)
}

pub fn secret(question: &str) -> Option<String> {
    line(question, "", true, |value| {
        if value.is_empty() {
            Err("Nothing was entered.".to_owned())
        } else {
            Ok(value.to_owned())
        }
    })
}

fn line(
    question: &str,
    placeholder: &str,
    hidden: bool,
    check: impl Fn(&str) -> Result<String, String>,
) -> Option<String> {
    if !interactive() {
        return None;
    }
    let Some(raw) = Raw::enter() else {
        return line_plainly(question, &check);
    };
    let mut value = String::new();
    let mut error: Option<String> = None;
    let mut screen = Screen::default();
    let accepted = loop {
        let shown = if value.is_empty() {
            style("2", placeholder)
        } else if hidden {
            "*".repeat(value.chars().count())
        } else {
            value.clone()
        };
        let mut lines = vec![format!("{} {} {shown}", style("36;1", "?"), style("1", question))];
        if let Some(error) = &error {
            lines.push(style("31", &format!("  {error}")));
        }
        screen.draw_with_cursor(&lines);
        match raw.key() {
            Key::Char(character) => {
                value.push(character);
                error = None;
            }
            Key::Backspace => {
                value.pop();
            }
            Key::ClearLine => value.clear(),
            Key::DeleteWord => {
                while value.ends_with(' ') {
                    value.pop();
                }
                while value.chars().last().is_some_and(|character| character != ' ') {
                    value.pop();
                }
            }
            Key::Enter => {
                let trimmed: String = value.chars().filter(|character| !character.is_whitespace()).collect();
                match check(&trimmed) {
                    Ok(accepted) => break Some(accepted),
                    Err(message) => error = Some(message),
                }
            }
            Key::Interrupt => interrupted(raw, &mut screen),
            Key::End | Key::Escape => break None,
            _ => {}
        }
    };
    let answer = match &accepted {
        None => style("2", "skipped"),
        Some(_) if hidden => style("36", "entered"),
        Some(accepted) => style("36", accepted),
    };
    screen.draw(&[format!("{} {} {answer}", style("36;1", "?"), style("1", question))]);
    write("\n");
    drop(raw);
    accepted
}

fn choose_plainly(question: &str, choices: &[Choice]) -> Option<usize> {
    eprintln!("{question}");
    for (index, choice) in choices.iter().enumerate() {
        eprintln!("  {}) {}: {}", index + 1, choice.label, choice.detail);
    }
    loop {
        let answer = read_line(&format!("[1-{}]: ", choices.len()))?;
        if let Ok(index) = answer.parse::<usize>()
            && (1..=choices.len()).contains(&index)
        {
            return Some(index - 1);
        }
    }
}

fn line_plainly(question: &str, check: &impl Fn(&str) -> Result<String, String>) -> Option<String> {
    loop {
        let answer = read_line(&format!("{question} "))?;
        match check(&answer) {
            Ok(accepted) => return Some(accepted),
            Err(message) => eprintln!("{message}"),
        }
    }
}

fn read_line(prompt: &str) -> Option<String> {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.chars().filter(|character| !character.is_whitespace()).collect()),
    }
}

fn interrupted(raw: Raw, screen: &mut Screen) -> ! {
    screen.draw(&[]);
    write("\x1b[?25h");
    drop(raw);
    eprintln!("Interrupted.");
    std::process::exit(130)
}

fn style(code: &str, text: &str) -> String {
    if colors() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

#[derive(Default)]
struct Screen {
    row: usize,
}

impl Screen {
    fn draw(&mut self, lines: &[String]) {
        let mut output = String::new();
        if self.row > 0 {
            output.push_str(&format!("\x1b[{}A", self.row));
        }
        output.push_str("\r\x1b[J");
        output.push_str(&lines.iter().map(|line| fit(line)).collect::<Vec<_>>().join("\n"));
        self.row = lines.len().saturating_sub(1);
        write(&output);
    }

    fn draw_with_cursor(&mut self, lines: &[String]) {
        self.draw(lines);
        if self.row > 0 {
            let width = visible_width(&fit(&lines[0]));
            write(&format!("\x1b[{}A\r\x1b[{width}C", self.row));
            self.row = 0;
        }
    }
}

fn fit(line: &str) -> String {
    let limit = columns().saturating_sub(1);
    let mut visible = 0;
    let mut fitted = String::new();
    let mut escape = false;
    for character in line.chars() {
        if escape {
            fitted.push(character);
            escape = character != 'm';
            continue;
        }
        if character == '\x1b' {
            escape = true;
            fitted.push(character);
            continue;
        }
        if visible == limit {
            continue;
        }
        visible += 1;
        fitted.push(character);
    }
    fitted
}

fn visible_width(line: &str) -> usize {
    let mut escape = false;
    line.chars()
        .filter(|character| {
            if escape {
                escape = *character != 'm';
                return false;
            }
            if *character == '\x1b' {
                escape = true;
                return false;
            }
            true
        })
        .count()
}

pub fn config_directory() -> PathBuf {
    if let Some(directory) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(directory).join("be3");
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".config/be3")
}

pub struct Saved<'a> {
    pub variable: &'a str,
    pub name: &'a str,
    pub what: &'a str,
    pub checkout: Option<&'a Path>,
}

impl Saved<'_> {
    pub fn path(&self) -> PathBuf {
        config_directory().join(self.name)
    }

    pub fn stored(&self) -> Option<String> {
        if let Ok(value) = std::env::var(self.variable) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
        let checkout = self.checkout.map(|root| root.join(format!(".{}", self.name)));
        checkout
            .into_iter()
            .chain([self.path()])
            .filter_map(|file| std::fs::read_to_string(file).ok())
            .map(|text| text.chars().filter(|character| !character.is_whitespace()).collect::<String>())
            .find(|value| !value.is_empty())
    }

    pub fn ask(&self, why: &str) -> Option<String> {
        if !interactive() {
            return None;
        }
        eprintln!("{why}");
        let value = secret(&format!("{}:", self.what))?;
        let path = self.path();
        match save_private(&path, &format!("{value}\n")) {
            Ok(()) => eprintln!("Saved it to {}; {} overrides it.", path.display(), self.variable),
            Err(error) => eprintln!("Could not save it to {}: {error}", path.display()),
        }
        Some(value)
    }
}

pub fn save_private(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(format!(".{}.partial", std::process::id()));
    let partial = PathBuf::from(partial);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&partial)?.write_all(text.as_bytes())?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests;
