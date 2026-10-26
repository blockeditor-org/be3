use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use be_server::store::ServerStore;

use crate::cores::Cores;
use crate::{ONE_WAY, latency};

pub const EMAIL: &str = "ada@example.com";
pub const PASSWORD: &str = "correct horse battery";
pub const SERVE: &str = "serve";

pub struct Server {
    pub address: SocketAddr,
    pub url: String,
    child: Option<Child>,
}

impl Server {
    pub fn start(build_info: &Path, data: &Path, cores: &Cores) -> Result<Self, String> {
        let me = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut command = Command::new(me);
        command
            .arg(SERVE)
            .arg(build_info)
            .arg(data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        cores.keep_to_server(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start the server: {error}"))?;
        let mut stdout = BufReader::new(child.stdout.take().ok_or("the server has no output")?);
        let mut line = String::new();
        let _ = stdout.read_line(&mut line);
        std::thread::spawn(move || std::io::copy(&mut stdout, &mut std::io::stderr()));
        let Some((address, proxy)) = line
            .split_once(' ')
            .and_then(|(address, proxy)| Some((address.parse().ok()?, proxy.trim())))
        else {
            drop(child.stdin.take());
            let status = child.wait().map_err(|error| error.to_string())?;
            return Err(format!(
                "the server ({status}) did not say where it listens: {line:?}"
            ));
        };
        Ok(Self {
            address,
            url: format!("http://{proxy}"),
            child: Some(child),
        })
    }

    pub fn approve(&self, code: &str) -> Result<(), String> {
        let mut browser = Browser {
            address: self.address,
            cookie: String::new(),
        };
        browser.submit(
            "/auth/login",
            "/auth/login",
            &[("email", EMAIL), ("password", PASSWORD)],
        )?;
        browser.submit(
            &format!("/auth?code={code}"),
            "/auth/approve",
            &[("code", code)],
        )?;
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }
}

pub fn serve(build_info: &Path, data: &Path) -> Result<(), String> {
    build_id::install_from(build_info)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let listener = runtime
        .block_on(async {
            let store = ServerStore::open(data)?;
            store.register(EMAIL, "Ada", PASSWORD).await?;
            Ok::<_, be_server::ServerError>(())
        })
        .map_err(|error| format!("could not make the account: {error}"))
        .and_then(|()| {
            runtime
                .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
                .map_err(|error| error.to_string())
        })?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let proxy = latency::start(address, ONE_WAY)
        .map_err(|error| format!("could not start the latency proxy: {error}"))?;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{address} {proxy}")
        .and_then(|()| stdout.flush())
        .map_err(|error| error.to_string())?;
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
        let _ = shutdown.send(());
    });
    runtime
        .block_on(be_server::serve_with_config(
            listener,
            data.to_path_buf(),
            be_server::ServerConfig::OPEN,
            stopped,
        ))
        .map_err(|error| format!("the server stopped: {error}"))
}

struct Browser {
    address: SocketAddr,
    cookie: String,
}

impl Browser {
    fn request(&mut self, method: &str, path: &str, body: &str) -> Result<(u16, String), String> {
        let address = self.address;
        let mut stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nCookie: be3-account={}\r\nContent-Length: {}\r\nContent-Type: application/x-www-form-urlencoded\r\nConnection: close\r\n\r\n{body}",
            self.cookie,
            body.len()
        )
        .map_err(|error| error.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;
        if let Some(cookie) = response.split("Set-Cookie: be3-account=").nth(1) {
            self.cookie = cookie.split(';').next().unwrap_or_default().to_owned();
        }
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|status| status.parse().ok())
            .ok_or_else(|| format!("the portal answered {method} {path} with {response:?}"))?;
        Ok((status, response))
    }

    fn submit(&mut self, page: &str, action: &str, fields: &[(&str, &str)]) -> Result<(), String> {
        let (_, form) = self.request("GET", page, "")?;
        let csrf = form
            .split("name=\"csrf\" value=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .ok_or_else(|| format!("{page} has no form: {form}"))?;
        let body = std::iter::once(("csrf", csrf))
            .chain(fields.iter().copied())
            .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        match self.request("POST", action, &body)? {
            (303, _) => Ok(()),
            (status, page) => Err(format!(
                "the portal answered {action} with {status}: {page}"
            )),
        }
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            byte => format!("%{byte:02X}"),
        })
        .collect()
}
