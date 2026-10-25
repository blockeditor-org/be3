use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};

use be_server::store::ServerStore;

pub const EMAIL: &str = "ada@example.com";
pub const PASSWORD: &str = "correct horse battery";

pub struct Server {
    pub address: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    pub fn start(build_info: &Path, data: PathBuf) -> Result<Self, String> {
        build_id::install_from(build_info)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let listener = runtime
            .block_on(async {
                let store = ServerStore::open(&data)?;
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
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("bench-server".into())
            .spawn(move || {
                runtime.block_on(async move {
                    if let Err(error) = be_server::serve_with_config(
                        listener,
                        data,
                        be_server::ServerConfig::OPEN,
                        stopped,
                    )
                    .await
                    {
                        panic!("the server stopped: {error}");
                    }
                });
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            address,
            shutdown: Some(shutdown),
            thread: Some(thread),
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
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
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
