use std::{io, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub mod host;
mod namespace;
pub mod products;
pub mod queue;

pub use namespace::{CLUSTER_KEY, LIFETIME, Waker};
use products::{Product, is_commit};
use queue::{Asked, Builds};

const MAX_HEAD_BYTES: usize = 16 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Service {
    pub waker: Option<Arc<Waker>>,
    pub builds: Option<Arc<Builds>>,
}

pub async fn serve(
    listener: TcpListener,
    service: Arc<Service>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let worker = service
        .builds
        .as_ref()
        .map(|builds| tokio::spawn(Arc::clone(builds).run()));
    tokio::pin!(shutdown);
    let result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => break Err(error),
                };
                let service = Arc::clone(&service);
                tokio::spawn(async move {
                    if let Err(error) = respond(stream, &service).await {
                        tracing::debug!("request ended: {error}");
                    }
                });
            }
        }
    };
    if let Some(worker) = worker {
        worker.abort();
    }
    result
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    headers: String,
    body: Vec<u8>,
}

impl Response {
    fn text(status: &'static str, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: String::new(),
            body: body.into().into_bytes(),
        }
    }

    fn json(status: &'static str, value: &impl serde::Serialize) -> Self {
        let mut body = serde_json::to_vec(value).unwrap();
        body.push(b'\n');
        Self {
            status,
            content_type: "application/json",
            headers: String::new(),
            body,
        }
    }

    fn unknown(error: Option<String>) -> Self {
        let mut value = serde_json::json!({"state": "unknown"});
        if let Some(error) = error {
            value["error"] = error.into();
        }
        Self::json("404 Not Found", &value)
    }
}

async fn respond(mut stream: TcpStream, service: &Service) -> io::Result<()> {
    let (method, target) = match tokio::time::timeout(READ_TIMEOUT, read_head(&mut stream)).await {
        Ok(head) => head?,
        Err(_) => return Ok(()),
    };
    let response = route(service, &method, &target).await;
    let head = format!(
        "HTTP/1.1 {}\r\ncontent-type: {}\r\ncontent-length: {}\r\n{}cache-control: no-store\r\nconnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len(),
        response.headers,
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&response.body).await?;
    stream.shutdown().await
}

async fn route(service: &Service, method: &str, target: &str) -> Response {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/build/namespace"
        && let Some(waker) = &service.waker
    {
        return namespace(waker, method).await;
    }
    if let (Some(rest), Some(builds)) = (path.strip_prefix("/build/products/"), &service.builds) {
        let parts: Vec<&str> = rest.split('/').collect();
        match parts[..] {
            [product, commit] => return products(builds, method, product, commit).await,
            [product, commit, "log"] if method == "GET" => {
                let from = query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("from="))
                    .and_then(|from| from.parse().ok())
                    .unwrap_or(0);
                return log(builds, product, commit, from).await;
            }
            _ => {}
        }
    }
    Response::text("404 Not Found", "Not found.\n")
}

async fn namespace(waker: &Waker, method: &str) -> Response {
    if method != "GET" {
        return Response::text(
            "405 Method Not Allowed",
            "GET asks for the cluster's hosts.\n",
        );
    }
    match waker.answer().await {
        Ok((text, age)) => Response {
            headers: format!("age: {}\r\n", age.as_secs()),
            ..Response::text("200 OK", text.to_string())
        },
        Err(error) => {
            tracing::error!("{error}");
            Response::text("502 Bad Gateway", format!("{error}\n"))
        }
    }
}

fn parse(product: &str, commit: &str) -> Result<Product, Response> {
    let product = Product::parse(product)
        .ok_or_else(|| Response::unknown(Some(format!("there is no product named {product}"))))?;
    if !is_commit(commit) {
        return Err(Response::unknown(Some(format!(
            "{commit} is not a full lowercase commit SHA"
        ))));
    }
    Ok(product)
}

async fn products(builds: &Arc<Builds>, method: &str, product: &str, commit: &str) -> Response {
    let product = match parse(product, commit) {
        Ok(product) => product,
        Err(response) => return response,
    };
    let (builds, commit) = (Arc::clone(builds), commit.to_owned());
    match method {
        "GET" => match tokio::task::spawn_blocking(move || builds.get(product, &commit)).await {
            Ok(Some(status)) => Response::json("200 OK", &status),
            Ok(None) => Response::unknown(None),
            Err(error) => Response::text("500 Internal Server Error", format!("{error}\n")),
        },
        "POST" => match tokio::task::spawn_blocking(move || builds.ask(product, &commit)).await {
            Ok(Asked::Status(status)) => Response::json("200 OK", &status),
            Ok(Asked::Refused(error)) => Response::json(
                "422 Unprocessable Entity",
                &serde_json::json!({"state": "refused", "error": error}),
            ),
            Err(error) => Response::text("500 Internal Server Error", format!("{error}\n")),
        },
        _ => Response::text(
            "405 Method Not Allowed",
            "GET reads a build's status and POST asks for it.\n",
        ),
    }
}

async fn log(builds: &Builds, product: &str, commit: &str, from: u64) -> Response {
    let product = match parse(product, commit) {
        Ok(product) => product,
        Err(response) => return response,
    };
    let Some(read) = builds.read_log(product, commit, from).await else {
        return Response::unknown(None);
    };
    let state = read.state.map_or("unknown", |state| state.name());
    Response {
        status: "200 OK",
        content_type: "text/plain; charset=utf-8",
        headers: format!(
            "build-state: {state}\r\nbuild-log-length: {}\r\n",
            read.length
        ),
        body: read.bytes,
    }
}

async fn read_head(stream: &mut TcpStream) -> io::Result<(String, String)> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        head.extend_from_slice(&chunk[..read]);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&head) {
            Ok(httparse::Status::Complete(_)) => {
                return Ok((
                    request.method.unwrap_or_default().to_owned(),
                    request.path.unwrap_or_default().to_owned(),
                ));
            }
            Ok(httparse::Status::Partial) if head.len() < MAX_HEAD_BYTES => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bad request head",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests;
