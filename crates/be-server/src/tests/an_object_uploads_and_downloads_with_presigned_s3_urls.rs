use std::{collections::HashMap, io::Read, time::Duration};

use rusty_s3::{Bucket, Credentials};

use super::*;
use crate::s3::{checksum, presign_get, presign_put};

pub(super) const EXPIRES: Duration = Duration::from_secs(600);

/// The bucket of `harness`'s S3 endpoint as a client reaches it through a proxy that serves
/// the server under `/api/be/`, and the credentials that sign for it.
pub(super) fn endpoint(harness: &Harness) -> (Bucket, Credentials) {
    let endpoint = ServerStore::open(&harness.directory)
        .unwrap()
        .s3_endpoint();
    let server = format!("{}/api/be/", harness.url).parse().unwrap();
    (endpoint.bucket(&server), endpoint.credentials())
}

pub(super) struct Answer {
    pub(super) status: u16,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Vec<u8>,
}

/// Sends one request with ureq, answering whatever status comes back.
pub(super) async fn send(
    method: &'static str,
    url: url::Url,
    headers: Vec<(&'static str, String)>,
    body: Option<Vec<u8>>,
) -> Answer {
    tokio::task::spawn_blocking(move || {
        let mut request = ureq::request(method, url.as_str());
        for (name, value) in &headers {
            request = request.set(name, value);
        }
        let response = match body {
            Some(body) => request.send_bytes(&body),
            None => request.call(),
        };
        let response = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => panic!("the request failed: {error}"),
        };
        let status = response.status();
        let headers = response
            .headers_names()
            .into_iter()
            .map(|name| {
                let value = response.header(&name).unwrap_or_default().to_owned();
                (name.to_ascii_lowercase(), value)
            })
            .collect();
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        Answer {
            status,
            headers,
            body,
        }
    })
    .await
    .unwrap()
}

/// Whether `answer` is an S3 error with `code`.
pub(super) fn refused_with(answer: &Answer, code: &str) -> bool {
    String::from_utf8_lossy(&answer.body).contains(&format!("<Code>{code}</Code>"))
}

pub(super) async fn upload(bucket: &Bucket, credentials: &Credentials, bytes: &[u8]) -> Answer {
    let hash = Hash::of(bytes);
    let url = presign_put(bucket, credentials, hash, bytes.len(), EXPIRES);
    send(
        "PUT",
        url,
        vec![("x-amz-checksum-sha256", checksum(hash))],
        Some(bytes.to_vec()),
    )
    .await
}

#[tokio::test]
async fn an_object_uploads_and_downloads_with_presigned_s3_urls() {
    let harness = Harness::start().await;
    let (bucket, credentials) = endpoint(&harness);
    let bytes = b"an object's sealed bytes".repeat(1000);
    let hash = Hash::of(&bytes);

    let put = upload(&bucket, &credentials, &bytes).await;
    assert_eq!(put.status, 200, "{}", String::from_utf8_lossy(&put.body));
    assert!(
        ServerStore::open(&harness.directory)
            .unwrap()
            .objects()
            .has(hash)
            .unwrap()
    );

    let got = send(
        "GET",
        presign_get(&bucket, &credentials, hash, EXPIRES),
        Vec::new(),
        None,
    )
    .await;
    assert_eq!(got.status, 200, "{}", String::from_utf8_lossy(&got.body));
    assert_eq!(got.body, bytes);
    assert_eq!(got.headers["access-control-allow-origin"], "*");
    assert_eq!(got.headers["cross-origin-resource-policy"], "cross-origin");

    let head = {
        use rusty_s3::S3Action;
        let key = hash.to_hex();
        let url = bucket
            .head_object(Some(&credentials), &key)
            .sign(EXPIRES);
        send("HEAD", url, Vec::new(), None).await
    };
    assert_eq!(head.status, 200);
    assert_eq!(head.headers["content-length"], bytes.len().to_string());
    assert!(head.body.is_empty());

    let missing = send(
        "GET",
        presign_get(&bucket, &credentials, Hash::of(b"never uploaded"), EXPIRES),
        Vec::new(),
        None,
    )
    .await;
    assert_eq!(missing.status, 404);
}
