use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use beui_core::app::automation::socket::{read_reply, write_request};
use beui_core::app::automation::{Reply, USAGE};

const TIMEOUT: Duration = Duration::from_secs(30);

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
    let mut timeout = TIMEOUT;
    let mut socket = std::env::var_os("BE_DRIVE_SOCKET").map(PathBuf::from);
    while let Some(first) = arguments.first() {
        if let Some(seconds) = first.strip_prefix("--timeout=") {
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
            "usage: be-drive [--timeout=SECONDS] [--socket=PATH] COMMAND [ARGUMENTS]\n\
             `shot` takes the file to write the PNG to first: shot FILE [TARGET].\n{USAGE}"
        ));
    }
    let socket = socket.ok_or(
        "BE_DRIVE_SOCKET is not set: source the env file the launcher printed, or pass --socket=PATH",
    )?;
    let mut file = None;
    if arguments[0] == "shot" {
        if arguments.len() < 2 {
            return Err("shot needs the file to write the PNG to: shot FILE [TARGET]".to_owned());
        }
        file = Some(PathBuf::from(arguments.remove(1)));
    }
    let stream = UnixStream::connect(&socket)
        .map_err(|error| format!("could not reach the app at {}: {error}", socket.display()))?;
    let mut writer = stream
        .try_clone()
        .map_err(|error| error.to_string())?;
    write_request(&mut writer, &arguments, timeout).map_err(|error| error.to_string())?;
    let reply = read_reply(&mut BufReader::new(stream))
        .map_err(|error| format!("the app did not answer: {error}"))?;
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
    }
}
