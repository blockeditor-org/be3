//! Serves the S3 endpoint (`crate::s3`): each request goes to s3s's own service, within the
//! largest object's length, and every answer carries what a browser on another origin needs.

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use http::{HeaderName, HeaderValue, Method, StatusCode, header};
use s3s::service::S3Service;
use tower::ServiceExt;
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};

use super::{BODY_READ_TIMEOUT, refuse};
use crate::{s3::BUCKET, store::MAX_OBJECT_BYTES};

/// Whether `path` (without its query) is the S3 endpoint's: the bucket or an object in it.
pub(super) fn is_s3(path: &str) -> bool {
    let bucket = format!("/{}{BUCKET}", crate::s3::PATH);
    path.ends_with(&bucket)
        || path
            .rsplit_once('/')
            .is_some_and(|(directory, _)| directory.ends_with(&bucket))
}

/// The endpoint over `service`. Every answer lets pages on any origin, even cross-origin
/// isolated ones, read objects with the URLs they are handed; the URL's signature is the only
/// credential.
pub(super) fn router(service: S3Service) -> Router {
    let header = |name: &'static str, value: &'static str| {
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        )
    };
    Router::new()
        .fallback(serve)
        .layer(RequestBodyLimitLayer::new(MAX_OBJECT_BYTES))
        .layer(tower_http::timeout::RequestBodyTimeoutLayer::new(
            BODY_READ_TIMEOUT,
        ))
        .layer(header("access-control-allow-origin", "*"))
        .layer(header(
            "access-control-expose-headers",
            "Content-Length, Content-Range, ETag, x-amz-checksum-sha256",
        ))
        .layer(header("cross-origin-resource-policy", "cross-origin"))
        .with_state(service)
}

async fn serve(State(service): State<S3Service>, request: Request) -> Response {
    if request.method() == Method::OPTIONS {
        return (
            StatusCode::NO_CONTENT,
            [
                (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, PUT"),
                (
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    "content-type, range, x-amz-checksum-sha256",
                ),
                (header::ACCESS_CONTROL_MAX_AGE, "86400"),
            ],
        )
            .into_response();
    }
    match service.oneshot(request).await {
        Ok(response) => response.map(Body::new),
        Err(error) => {
            tracing::warn!(?error, "the S3 endpoint failed a request");
            super::closing(refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the request failed",
            ))
        }
    }
}
