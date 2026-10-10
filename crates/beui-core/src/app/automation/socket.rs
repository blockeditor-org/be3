use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use super::{Inbox, Reply, Request};

pub fn serve(path: &Path, inbox: Inbox) -> io::Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    let path = path.to_path_buf();
    std::thread::Builder::new()
        .name("beui automation".to_owned())
        .spawn(move || listen(listener, path, inbox))?;
    Ok(())
}

fn listen(listener: UnixListener, path: PathBuf, inbox: Inbox) {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("beui: automation socket {}: {error}", path.display());
                return;
            }
        };
        let inbox = inbox.clone();
        let _ = std::thread::Builder::new()
            .name("beui automation client".to_owned())
            .spawn(move || {
                if let Err(error) = converse(stream, &inbox)
                    && error.kind() != io::ErrorKind::UnexpectedEof
                {
                    eprintln!("beui: automation client: {error}");
                }
            });
    }
}

fn converse(stream: UnixStream, inbox: &Inbox) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    loop {
        let Some((words, timeout)) = read_request(&mut reader)? else {
            return Ok(());
        };
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        inbox.send(Request {
            words,
            reply: sender,
            cancelled: cancelled.clone(),
        });
        let reply = match receiver.recv_timeout(timeout) {
            Ok(reply) => reply,
            Err(RecvTimeoutError::Timeout) => {
                cancelled.store(true, Ordering::SeqCst);
                Reply::Error(format!(
                    "timed out after {} seconds",
                    timeout.as_secs_f32()
                ))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Reply::Error("the app stopped before it answered".to_owned())
            }
        };
        write_reply(&mut writer, &reply)?;
    }
}

pub fn read_request(reader: &mut impl BufRead) -> io::Result<Option<(Vec<String>, Duration)>> {
    let mut header = String::new();
    if reader.read_line(&mut header)? == 0 {
        return Ok(None);
    }
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "malformed request header");
    let mut fields = header.split_whitespace();
    let length: usize = fields
        .next()
        .and_then(|field| field.parse().ok())
        .ok_or_else(invalid)?;
    let timeout: u64 = fields
        .next()
        .and_then(|field| field.parse().ok())
        .ok_or_else(invalid)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let body = String::from_utf8(body).map_err(|_| invalid())?;
    let words = body.split('\0').map(str::to_owned).collect();
    Ok(Some((words, Duration::from_millis(timeout))))
}

pub fn write_request(writer: &mut impl Write, words: &[String], timeout: Duration) -> io::Result<()> {
    let body = words.join("\0");
    write!(writer, "{} {}\n", body.len(), timeout.as_millis())?;
    writer.write_all(body.as_bytes())?;
    writer.flush()
}

pub fn write_reply(writer: &mut impl Write, reply: &Reply) -> io::Result<()> {
    match reply {
        Reply::Text(text) => {
            write!(writer, "text {}\n", text.len())?;
            writer.write_all(text.as_bytes())?;
        }
        Reply::Error(text) => {
            write!(writer, "error {}\n", text.len())?;
            writer.write_all(text.as_bytes())?;
        }
        Reply::Image {
            width,
            height,
            rgba,
        } => {
            write!(writer, "image {width} {height}\n")?;
            writer.write_all(rgba)?;
        }
    }
    writer.flush()
}

pub fn read_reply(reader: &mut impl BufRead) -> io::Result<Reply> {
    let mut header = String::new();
    if reader.read_line(&mut header)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "malformed reply header");
    let fields: Vec<&str> = header.split_whitespace().collect();
    let number = |index: usize| -> io::Result<usize> {
        fields
            .get(index)
            .and_then(|field| field.parse().ok())
            .ok_or_else(invalid)
    };
    let mut read = |length: usize| -> io::Result<Vec<u8>> {
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        Ok(body)
    };
    match fields.first().copied() {
        Some("text") => Ok(Reply::Text(
            String::from_utf8(read(number(1)?)?).map_err(|_| invalid())?,
        )),
        Some("error") => Ok(Reply::Error(
            String::from_utf8(read(number(1)?)?).map_err(|_| invalid())?,
        )),
        Some("image") => {
            let (width, height) = (number(1)?, number(2)?);
            Ok(Reply::Image {
                width: width as u32,
                height: height as u32,
                rgba: read(width * height * 4)?,
            })
        }
        _ => Err(invalid()),
    }
}
