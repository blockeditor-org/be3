mod launch;

use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use beui_core::app::automation::socket::{read_reply, write_request};
use beui_core::app::automation::{Reply, USAGE};

const TIMEOUT: Duration = Duration::from_secs(30);
const ANSWER_GRACE: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
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
    if arguments.first().map(String::as_str) == Some("launch") {
        return launch::run(&arguments[1..]);
    }
    let mut timeout = TIMEOUT;
    let mut socket = std::env::var_os("BE_DRIVE_SOCKET").map(PathBuf::from);
    let mut options = Vec::new();
    while let Some(first) = arguments.first() {
        if first == "--no-settle" || first.starts_with("--changes=") {
            options.push(first.clone());
        } else if let Some(seconds) = first.strip_prefix("--timeout=") {
            let seconds: f64 = seconds
                .parse()
                .map_err(|_| format!("--timeout takes seconds, not {seconds}"))?;
            timeout = Duration::from_secs_f64(seconds);
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
             `-` reads one command a line from standard input, quoted as a shell would, and stops at the first that fails.\n\
             {USAGE}"
        ));
    }
    let socket = socket.ok_or(
        "BE_DRIVE_SOCKET is not set: source the env file the launcher printed, or pass --socket=PATH",
    )?;
    let settled = |words: Vec<String>| options.iter().cloned().chain(words).collect::<Vec<_>>();
    if arguments == ["-"] {
        let mut script = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut script)
            .map_err(|error| format!("could not read the commands: {error}"))?;
        for line in script.lines() {
            let words = words(line)?;
            if words.is_empty() || words[0].starts_with('#') {
                continue;
            }
            println!("> {}", line.trim());
            command(&socket, settled(words), timeout)?;
        }
        return Ok(());
    }
    command(&socket, settled(arguments), timeout)
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
        Some("upload") if arguments.len() > first + 1 => {
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
        file = Some(PathBuf::from(arguments.remove(first + 1)));
    }
    match request(socket, &arguments, timeout)? {
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
    }
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

pub(crate) fn request(socket: &Path, words: &[String], timeout: Duration) -> Result<Reply, String> {
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
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => format!(
            "the app did not answer within {} seconds; is it stuck? Its log says more",
            deadline.as_secs()
        ),
        _ => format!("the app did not answer: {error}"),
    })?;
    match reply {
        Reply::Error(error) => Err(error),
        reply => Ok(reply),
    }
}
