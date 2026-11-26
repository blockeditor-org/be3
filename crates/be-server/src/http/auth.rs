//! The account portal's pages, forms and assets (`crate::portal`), at `/auth`.

use std::{collections::HashMap, net::IpAddr};

use axum::{
    Extension,
    body::Body,
    extract::State,
    response::{IntoResponse, Response},
};
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};

use super::{Admission, closing, failed, read, refuse, text};
use crate::{HANDSHAKE_TIMEOUT, ServerError, Services};

/// A form's body.
pub(super) const MAX_FORM_BYTES: usize = 16 * 1024;

/// Serves one request to the portal, then closes.
pub(super) async fn serve(
    State(services): State<Services>,
    Extension(Admission { client, .. }): Extension<Admission>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    closing(
        portal(&services, client, &method, &uri, &headers, body)
            .await
            .unwrap_or_else(failed),
    )
}

async fn portal(
    services: &Services,
    client: IpAddr,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: Body,
) -> Result<Response, ServerError> {
    let host = text(headers, "host").unwrap_or_default();
    let local = host == "localhost"
        || host.starts_with("localhost:")
        || host == "127.0.0.1"
        || host.starts_with("127.0.0.1:")
        || host == "[::1]"
        || host.starts_with("[::1]:");
    let origin = services
        .config
        .auth_origin
        .clone()
        .unwrap_or_else(|| format!("{}://{host}", if local { "http" } else { "https" }));
    if !local && services.config.auth_origin.is_none() {
        return Ok(refuse(
            StatusCode::SERVICE_UNAVAILABLE,
            "The server operator must configure --auth-origin before using the account portal.",
        ));
    }
    let path = uri.path();
    if method == Method::GET
        && let Some(asset) = services
            .portal
            .as_ref()
            .and_then(|served| served.asset(path))
    {
        return Ok((
            [
                (header::CONTENT_TYPE, asset.content_type),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                (
                    header::HeaderName::from_static("cross-origin-resource-policy"),
                    "same-origin",
                ),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            asset.bytes,
        )
            .into_response());
    }
    let query = crate::browser_auth::fields(uri.query().unwrap_or_default());
    let mut fields = HashMap::new();
    if method == Method::POST {
        if text(headers, "origin") != Some(origin.as_str()) {
            return Ok(refuse(
                StatusCode::FORBIDDEN,
                "submit this form from the server's account portal",
            ));
        }
        if text(headers, "content-type").and_then(|value| value.split(';').next())
            != Some("application/x-www-form-urlencoded")
        {
            return Ok(refuse(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "use the account portal's form",
            ));
        }
        let body = match read(body, Some(HANDSHAKE_TIMEOUT)).await {
            Ok(body) => body,
            Err(refused) => return Ok(refused),
        };
        fields = crate::browser_auth::fields(std::str::from_utf8(&body).unwrap_or_default());
    }
    let cookies = PortalCookies::for_origin(&origin);
    let cookie = |name: &str| {
        text(headers, "cookie")
            .unwrap_or_default()
            .split(';')
            .filter_map(|part| part.trim().split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
            .unwrap_or_default()
    };
    let token = cookie(&cookies.name(ACCOUNT_COOKIE));
    let attempt = cookie(&cookies.name(ATTEMPT_COOKIE));
    let result = crate::portal::handle(
        services,
        method.as_str(),
        path,
        &query,
        &fields,
        token,
        attempt,
        &origin,
        client,
    )
    .await?;
    let policy = match &result.body {
        crate::portal::Body::Html(_) => services
            .portal
            .as_ref()
            .map(|served| served.policy.as_str())
            .unwrap_or_default(),
        crate::portal::Body::Json(_) => "default-src 'none'; frame-ancestors 'none'",
    };
    let mut response = match result.body {
        crate::portal::Body::Html(html) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (
                    header::HeaderName::from_static("cross-origin-opener-policy"),
                    "same-origin",
                ),
                (
                    header::HeaderName::from_static("cross-origin-embedder-policy"),
                    "require-corp",
                ),
            ],
            html,
        )
            .into_response(),
        crate::portal::Body::Json(json) => {
            ([(header::CONTENT_TYPE, "application/json")], json).into_response()
        }
    };
    *response.status_mut() =
        StatusCode::from_u16(result.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let headers = response.headers_mut();
    let mut insert = |name: header::HeaderName, value: &str| {
        if let Ok(value) = HeaderValue::from_str(value) {
            headers.append(name, value);
        }
    };
    insert(header::CONTENT_SECURITY_POLICY, policy);
    insert(header::CACHE_CONTROL, "no-store");
    insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert(header::REFERRER_POLICY, "strict-origin");
    if let Some(location) = result.location {
        insert(header::LOCATION, &location);
    }
    if let Some(cookie) = result.cookie {
        let lifetime = if cookie.is_empty() { 0 } else { 30 * 86400 };
        insert(
            header::SET_COOKIE,
            &cookies.set(ACCOUNT_COOKIE, &cookie, lifetime),
        );
    }
    if let Some(attempt) = result.attempt {
        let lifetime = match attempt.is_empty() {
            true => 0,
            false => crate::store::codes::CODE_LIFETIME,
        };
        insert(
            header::SET_COOKIE,
            &cookies.set(ATTEMPT_COOKIE, &attempt, lifetime),
        );
    }
    Ok(response)
}

/// The portal's session cookie and its code attempt's (`codes.rs`).
const ACCOUNT_COOKIE: &str = "be3-account";
const ATTEMPT_COOKIE: &str = "be3-attempt";

/// How the portal names and scopes its cookies at its origin. Over HTTPS each is a `__Host-`
/// cookie (Secure, `Path=/`, no `Domain`), which a sibling subdomain can neither set nor
/// shadow. Plain HTTP, only for a server on this machine, can't set `__Host-` (it needs
/// Secure), so there the bare name is scoped to `/auth`: nothing else at that origin to shadow
/// it.
struct PortalCookies {
    secure: bool,
}

impl PortalCookies {
    fn for_origin(origin: &str) -> Self {
        Self {
            secure: origin.starts_with("https://"),
        }
    }

    fn name(&self, base: &str) -> String {
        match self.secure {
            true => format!("__Host-{base}"),
            false => base.to_owned(),
        }
    }

    /// The `Set-Cookie` value that sets `base` to `value` for `max_age` seconds (0 ends it).
    fn set(&self, base: &str, value: &str, max_age: i64) -> String {
        let (path, secure) = match self.secure {
            true => ("/", "; Secure"),
            false => ("/auth", ""),
        };
        format!(
            "{}={value}; Path={path}; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}",
            self.name(base)
        )
    }
}
