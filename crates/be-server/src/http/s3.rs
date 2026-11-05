//! Serves the S3 endpoint (`crate::s3`) on the server's HTTP connections: each request's head and
//! body are read within the server's limits, handed to s3s as `http` types, and answered with
//! what a browser on another origin needs.

use bytes::Bytes;
use futures_util::StreamExt;
use s3s::service::S3Service;
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::{IDLE_TIMEOUT, MAX_HEADERS, read_body, read_head};
use crate::{ServerError, s3::BUCKET, store::MAX_OBJECT_BYTES};

/// Headers every answer carries, so pages on any origin, even cross-origin isolated ones, can
/// read objects with the URLs they are handed; the URL's signature is the only credential.
const CROSS_ORIGIN: &str = "Access-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: Content-Length, Content-Range, ETag, x-amz-checksum-sha256\r\nCross-Origin-Resource-Policy: cross-origin\r\n";

/// Whether `path` (without its query) is the S3 endpoint's: the bucket or an object in it.
pub(super) fn is_s3(path: &str) -> bool {
    let bucket = format!("/{}{BUCKET}", crate::s3::PATH);
    path.ends_with(&bucket)
        || path
            .rsplit_once('/')
            .is_some_and(|(directory, _)| directory.ends_with(&bucket))
}

/// Serves S3 requests on `stream`, whose first head is in `buffer`, until the client closes,
/// asks to, or sends a request for anything else.
pub(super) async fn serve(
    mut stream: TcpStream,
    mut buffer: Vec<u8>,
    service: &S3Service,
) -> Result<(), ServerError> {
    loop {
        let Some(head) = Head::parse(&buffer)? else {
            return answer(&mut stream, 400, "the request head is malformed", true).await;
        };
        let path = head.request.uri().path();
        if !is_s3(path) {
            return answer(&mut stream, 404, "only S3 requests on this connection", true).await;
        }
        if head.chunked {
            return answer(&mut stream, 411, "send a Content-Length", true).await;
        }
        if head.length > MAX_OBJECT_BYTES {
            return answer(&mut stream, 413, "an object is at most 8 MiB", true).await;
        }
        let mut body = buffer.split_off(head.size);
        if !read_body(&mut stream, &mut body, head.length).await? {
            return Ok(());
        }
        buffer = body.split_off(head.length);
        if head.request.method() == http::Method::OPTIONS {
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 204 No Content\r\n{CROSS_ORIGIN}Access-Control-Allow-Methods: GET, HEAD, PUT\r\nAccess-Control-Allow-Headers: content-type, range, x-amz-checksum-sha256\r\nAccess-Control-Max-Age: 86400\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await?;
        } else {
            let (parts, ()) = head.request.into_parts();
            let request = http::Request::from_parts(parts, s3s::Body::from(body));
            match service.call(request).await {
                Ok(response) => respond(&mut stream, response, head.close).await?,
                Err(error) => {
                    tracing::warn!(?error, "the S3 endpoint failed a request");
                    return answer(&mut stream, 500, "the request failed", true).await;
                }
            }
        }
        if head.close {
            stream.shutdown().await?;
            return Ok(());
        }
        stream.flush().await?;
        match tokio::time::timeout(IDLE_TIMEOUT, read_head(&mut stream, &mut buffer)).await {
            Ok(Ok(true)) => {}
            Ok(Err(error)) => return Err(error),
            Ok(Ok(false)) | Err(_) => return Ok(()),
        }
    }
}

struct Head {
    request: http::Request<()>,
    size: usize,
    length: usize,
    chunked: bool,
    close: bool,
}

impl Head {
    fn parse(buffer: &[u8]) -> Result<Option<Self>, ServerError> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut parsed = httparse::Request::new(&mut headers);
        let size = match parsed.parse(buffer) {
            Ok(httparse::Status::Complete(size)) => size,
            _ => return Ok(None),
        };
        let mut request = http::Request::builder()
            .method(parsed.method.unwrap_or_default())
            .uri(parsed.path.unwrap_or_default());
        let (mut length, mut chunked, mut close) = (0, false, false);
        for header in parsed.headers.iter() {
            if header.name.eq_ignore_ascii_case("content-length") {
                let Some(parsed) = std::str::from_utf8(header.value)
                    .ok()
                    .and_then(|value| value.trim().parse().ok())
                else {
                    return Ok(None);
                };
                length = parsed;
            } else if header.name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = true;
            } else if header.name.eq_ignore_ascii_case("connection") {
                close = header.value.eq_ignore_ascii_case(b"close");
            }
            request = request.header(header.name, header.value);
        }
        Ok(request.body(()).ok().map(|request| Self {
            request,
            size,
            length,
            chunked,
            close,
        }))
    }
}

async fn respond(
    stream: &mut TcpStream,
    response: http::Response<s3s::Body>,
    close: bool,
) -> Result<(), ServerError> {
    let (parts, mut body) = response.into_parts();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk: Bytes =
            chunk.map_err(|error| ServerError::Socket(format!("an S3 answer failed: {error}")))?;
        bytes.extend_from_slice(&chunk);
    }
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n{CROSS_ORIGIN}",
        parts.status.as_u16(),
        parts.status.canonical_reason().unwrap_or("Response")
    );
    for (name, value) in &parts.headers {
        if [
            http::header::CONNECTION,
            http::header::TRANSFER_ENCODING,
            http::header::CONTENT_LENGTH,
        ]
        .contains(name)
        {
            continue;
        }
        head.push_str(&format!(
            "{name}: {}\r\n",
            String::from_utf8_lossy(value.as_bytes())
        ));
    }
    // A HEAD answer states the object's length and carries no body.
    let length = parts
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .map_or_else(|| bytes.len().to_string(), str::to_owned);
    head.push_str(&format!("Content-Length: {length}\r\n"));
    if close {
        head.push_str("Connection: close\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn answer(
    stream: &mut TcpStream,
    status: u16,
    message: &str,
    close: bool,
) -> Result<(), ServerError> {
    let reason = http::StatusCode::from_u16(status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or("Error");
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status} {reason}\r\n{CROSS_ORIGIN}Content-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\n{}\r\n{message}",
                message.len(),
                if close { "Connection: close\r\n" } else { "" }
            )
            .as_bytes(),
        )
        .await?;
    if close {
        stream.shutdown().await?;
    }
    Ok(())
}
