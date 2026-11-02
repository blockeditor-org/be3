use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const LOCK: &str = "instance.lock";
const ADDRESS: &str = "instance";
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
const SHOWN: &str = "shown";

pub(crate) enum Claim {
    First(Instance),
    Running(Running),
}

pub(crate) struct Instance {
    _lock: File,
    listener: TcpListener,
    token: String,
}

pub(crate) struct Running {
    data_dir: PathBuf,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Handoff {
    pub(crate) dev_workspace: bool,
}

pub(crate) fn claim(data_dir: &Path) -> Result<Claim, String> {
    let path = data_dir.join(LOCK);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Ok(Claim::Running(Running {
                data_dir: data_dir.to_owned(),
            }));
        }
        Err(TryLockError::Error(error)) => {
            return Err(format!("could not lock {}: {error}", path.display()));
        }
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("could not listen for a second be-host: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = uuid::Uuid::new_v4().simple().to_string();
    let written = data_dir.join(format!("{ADDRESS}.new"));
    std::fs::write(&written, format!("{port} {token}\n"))
        .and_then(|()| std::fs::rename(&written, data_dir.join(ADDRESS)))
        .map_err(|error| {
            format!(
                "could not write {}: {error}",
                data_dir.join(ADDRESS).display()
            )
        })?;
    Ok(Claim::First(Instance {
        _lock: lock,
        listener,
        token,
    }))
}

impl Instance {
    pub(crate) fn serve(self, handed: impl Fn(Handoff) + Send + 'static) {
        std::thread::spawn(move || {
            let instance = self;
            for stream in instance.listener.incoming() {
                let Ok(stream) = stream else {
                    continue;
                };
                if let Err(error) = answer(stream, &instance.token, &handed) {
                    eprintln!("be-host: a second be-host was not answered: {error}");
                }
            }
        });
    }
}

fn answer(stream: TcpStream, token: &str, handed: &impl Fn(Handoff)) -> io::Result<()> {
    stream.set_read_timeout(Some(ANSWER_WITHIN))?;
    stream.set_write_timeout(Some(ANSWER_WITHIN))?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let handoff = parse(&line, token).map_err(io::Error::other)?;
    handed(handoff);
    writeln!(&stream, "{SHOWN}")
}

impl Running {
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub(crate) fn hand_over(&self, handoff: Handoff) -> Result<(), String> {
        let path = self.data_dir.join(ADDRESS);
        let address = std::fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let (port, token) = address
            .trim()
            .split_once(' ')
            .and_then(|(port, token)| Some((port.parse::<u16>().ok()?, token)))
            .ok_or_else(|| format!("{} does not hold a port and a token", path.display()))?;
        let stream = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            ANSWER_WITHIN,
        )
        .map_err(|error| format!("could not reach it: {error}"))?;
        let talk = || -> io::Result<String> {
            stream.set_read_timeout(Some(ANSWER_WITHIN))?;
            stream.set_write_timeout(Some(ANSWER_WITHIN))?;
            writeln!(&stream, "{}", request(token, handoff))?;
            let mut answer = String::new();
            BufReader::new(&stream).read_line(&mut answer)?;
            Ok(answer)
        };
        let answer = talk().map_err(|error| format!("it did not answer: {error}"))?;
        match answer.trim() {
            SHOWN => Ok(()),
            other => Err(format!("it answered {other:?}")),
        }
    }
}

pub(crate) fn request(token: &str, handoff: Handoff) -> String {
    let mut words = vec![token, "show"];
    if handoff.dev_workspace {
        words.push("dev-workspace");
    }
    words.join(" ")
}

pub(crate) fn parse(line: &str, token: &str) -> Result<Handoff, String> {
    let mut words = line.split_whitespace();
    if words.next() != Some(token) {
        return Err("it did not know this be-host's token".to_owned());
    }
    if words.next() != Some("show") {
        return Err(format!("it asked for something other than show: {line:?}"));
    }
    let mut handoff = Handoff::default();
    for word in words {
        match word {
            "dev-workspace" => handoff.dev_workspace = true,
            other => return Err(format!("it asked for {other:?}, which be-host does not know")),
        }
    }
    Ok(handoff)
}

#[cfg(test)]
mod tests;
