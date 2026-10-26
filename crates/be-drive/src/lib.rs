mod driven;
mod launch;

use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub use driven::Driven;
use std::time::Duration;

use beui_core::app::automation::socket::{read_reply, write_request};
use beui_core::app::automation::{Reply, USAGE};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);
const ANSWER_GRACE: Duration = Duration::from_secs(5);

pub fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let mut stderr = std::io::stderr();
            let _ = writeln!(stderr, "{}", error.trim_end());
            ExitCode::FAILURE
        }
    }
}

fn run(mut arguments: Vec<String>) -> Result<(), String> {
    match arguments.first().map(String::as_str) {
        Some("launch") => return launch::run(&arguments[1..]),
        Some("run") => return launch::run_script(&arguments[1..]),
        _ => {}
    }
    let mut timeout = TIMEOUT;
    let mut socket = std::env::var_os("BE_DRIVE_SOCKET").map(PathBuf::from);
    let mut options = Vec::new();
    while let Some(first) = arguments.first() {
        if first == "--no-settle" || first.starts_with("--changes=") {
            options.push(first.clone());
        } else if let Some(seconds) = first.strip_prefix("--timeout=") {
            timeout = seconds
                .parse()
                .ok()
                .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
                .ok_or_else(|| format!("--timeout takes seconds, not {seconds}"))?;
        } else if let Some(path) = first.strip_prefix("--socket=") {
            socket = Some(PathBuf::from(path));
        } else {
            break;
        }
        arguments.remove(0);
    }
    if arguments.is_empty() || arguments[0] == "--help" {
        return Err(format!(
            "usage: drive [--timeout=SECONDS] [--socket=PATH] [--no-settle] [--changes=N|all] COMMAND [ARGUMENTS]\n\
             \x20      drive [--timeout=SECONDS] [--socket=PATH] - < COMMANDS\n\
             `shot` takes the file to write the PNG to first: shot FILE [TARGET].\n\
             `paint` takes the .paint file to write the paint snapshot to first: paint FILE.paint [TARGET].\n\
             `-` reads one command a line from standard input, quoted as a shell would, and stops at the first that fails.\n\
             `env` prints the app's environment for programs as export lines, for `eval \"$(drive env)\"`.\n\
             `exec PROGRAM [ARGUMENTS]` runs PROGRAM in that environment, so its windows open inside the app.\n\
             {USAGE}"
        ));
    }
    let socket = socket.ok_or(
        "BE_DRIVE_SOCKET is not set: source the env file the launcher printed, or pass --socket=PATH",
    )?;
    if arguments == ["-"] {
        return script(&socket, &options, timeout);
    }
    if arguments[0] == "exec" {
        return exec(&socket, &arguments[1..], timeout);
    }
    command(
        &socket,
        options.into_iter().chain(arguments).collect(),
        timeout,
    )
}

fn written(file: String, extension: &str, command: &str) -> Result<PathBuf, String> {
    let file = PathBuf::from(file);
    match file.extension().is_some_and(|found| found == extension) {
        true => Ok(file),
        false => Err(format!(
            "{command} writes to a .{extension} file named first, {command} FILE [TARGET]; {} is not one",
            file.display()
        )),
    }
}

pub(crate) fn script(socket: &Path, options: &[String], timeout: Duration) -> Result<(), String> {
    let mut script = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut script)
        .map_err(|error| format!("could not read the commands: {error}"))?;
    for line in script.lines() {
        let words = words(line)?;
        if words.is_empty() || words[0].starts_with('#') {
            continue;
        }
        println!("> {}", line.trim());
        command(
            socket,
            options.iter().cloned().chain(words).collect(),
            timeout,
        )?;
    }
    Ok(())
}

fn command(socket: &Path, mut arguments: Vec<String>, timeout: Duration) -> Result<(), String> {
    let first = arguments
        .iter()
        .take_while(|word| *word == "--no-settle" || word.starts_with("--changes="))
        .count();
    let absolute = |path: &String| {
        std::path::absolute(path)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| path.clone())
    };
    match arguments.get(first).map(String::as_str) {
        Some("upload" | "save") if arguments.len() > first + 1 => {
            arguments[first + 1] = absolute(&arguments[first + 1]);
        }
        Some("grab") => {
            for file in &mut arguments[first + 1..] {
                *file = absolute(file);
            }
        }
        _ => {}
    }
    let mut file = None;
    if arguments.get(first).map(String::as_str) == Some("shot") {
        if arguments.len() < first + 2 {
            return Err("shot needs the file to write the PNG to: shot FILE [TARGET]".to_owned());
        }
        file = Some(written(arguments.remove(first + 1), "png", "shot")?);
    }
    if arguments.get(first).map(String::as_str) == Some("paint") {
        if arguments.len() < first + 2 {
            return Err(
                "paint needs the file to write the painting to: paint FILE [TARGET]".to_owned(),
            );
        }
        file = Some(written(arguments.remove(first + 1), "paint", "paint")?);
    }
    if arguments.get(first).map(String::as_str) == Some("env") {
        for (name, value) in environment(socket, timeout)? {
            println!("export {name}={}", quoted(&value));
        }
        return Ok(());
    }
    let closing = arguments.get(first).map(String::as_str) == Some("close");
    let reply = match request(socket, &arguments, timeout) {
        Err(Unanswered::Closed) if closing => Reply::Text("The window closed.\n".to_owned()),
        Err(unanswered) => return Err(unanswered.to_string()),
        Ok(reply) => reply,
    };
    match reply {
        Reply::Text(text) => {
            print!("{text}");
            Ok(())
        }
        Reply::Error(error) => Err(error),
        Reply::Image {
            width,
            height,
            rgba,
        } => {
            let file = file.ok_or("the app sent an image that was not asked for")?;
            let image = image::RgbaImage::from_raw(width, height, rgba)
                .ok_or("the app sent an image of the wrong size")?;
            image
                .save(&file)
                .map_err(|error| format!("could not write {}: {error}", file.display()))?;
            println!("{} ({width}x{height})", file.display());
            Ok(())
        }
        Reply::Painting(bytes) => {
            let file = file.ok_or("the app sent a painting that was not asked for")?;
            let painting = paint_snapshot::Painting::decode(&bytes)?;
            let encoded = paint_snapshot::Snapshot::of_painting(painting)?.encode()?;
            std::fs::write(&file, encoded)
                .map_err(|error| format!("could not write {}: {error}", file.display()))?;
            println!("{}", file.display());
            Ok(())
        }
    }
}

fn environment(socket: &Path, timeout: Duration) -> Result<Vec<(String, String)>, String> {
    match request(socket, &["env".to_owned()], timeout).map_err(|error| error.to_string())? {
        Reply::Text(text) => Ok(text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect()),
        _ => Err("the app answered env with something other than text".to_owned()),
    }
}

fn exec(socket: &Path, arguments: &[String], timeout: Duration) -> Result<(), String> {
    let arguments = match arguments.first().map(String::as_str) {
        Some("--") => &arguments[1..],
        _ => arguments,
    };
    let (program, arguments) = arguments
        .split_first()
        .ok_or("exec needs a program to run: exec PROGRAM [ARGUMENTS]")?;
    let environment = environment(socket, timeout)?;
    if environment.is_empty() {
        return Err(
            "the app hosts no programs; block-app does with --desktop (guides/linux_desktop.md)"
                .to_owned(),
        );
    }
    let error = std::process::Command::new(program)
        .args(arguments)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("BEUI_AUTOMATION")
        .env_remove("BEUI_AUTOMATION_LISTENER")
        .env_remove("BEUI_HEADLESS")
        .env_remove("BEUI_HEADLESS_SCREEN")
        .envs(environment)
        .exec();
    Err(format!("could not run {program}: {error}"))
}

fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        match character {
            '\'' => {
                started = true;
                loop {
                    match characters.next() {
                        Some('\'') => break,
                        Some(character) => word.push(character),
                        None => return Err(format!("an unclosed quote in: {line}")),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match characters.next() {
                        Some('"') => break,
                        Some('\\') => word.extend(characters.next()),
                        Some(character) => word.push(character),
                        None => return Err(format!("an unclosed quote in: {line}")),
                    }
                }
            }
            '\\' => {
                started = true;
                word.extend(characters.next());
            }
            character if character.is_whitespace() => {
                if std::mem::take(&mut started) {
                    words.push(std::mem::take(&mut word));
                }
            }
            character => {
                started = true;
                word.push(character);
            }
        }
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

pub(crate) enum Unanswered {
    Closed,
    Failed(String),
}

impl std::fmt::Display for Unanswered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => formatter.write_str("the app closed the connection without answering"),
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}

impl From<String> for Unanswered {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

pub(crate) fn request(
    socket: &Path,
    words: &[String],
    timeout: Duration,
) -> Result<Reply, Unanswered> {
    let stream = UnixStream::connect(socket)
        .map_err(|error| format!("could not reach the app at {}: {error}", socket.display()))?;
    let deadline = timeout + ANSWER_GRACE;
    stream
        .set_read_timeout(Some(deadline))
        .and_then(|()| stream.set_write_timeout(Some(deadline)))
        .map_err(|error| error.to_string())?;
    let mut writer = stream.try_clone().map_err(|error| error.to_string())?;
    write_request(&mut writer, words, timeout).map_err(|error| error.to_string())?;
    let reply = read_reply(&mut BufReader::new(stream)).map_err(|error| match error.kind() {
        std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset => {
            Unanswered::Closed
        }
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
            Unanswered::Failed(format!(
                "the app did not answer within {} seconds; is it stuck? Its log says more",
                deadline.as_secs()
            ))
        }
        _ => Unanswered::Failed(format!("the app did not answer: {error}")),
    })?;
    match reply {
        Reply::Error(error) => Err(Unanswered::Failed(error)),
        reply => Ok(reply),
    }
}
