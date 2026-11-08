use serde::Serialize;

use crate::network::{Header, Request, ResponseHead};

impl Request {
    /// A request with no headers and no body.
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new("POST", url)
    }

    pub fn put(url: impl Into<String>) -> Self {
        Self::new("PUT", url)
    }

    pub fn patch(url: impl Into<String>) -> Self {
        Self::new("PATCH", url)
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self::new("DELETE", url)
    }

    /// Adds a header; a name given twice is sent twice.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(Header {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    /// Sends `token` as an `Authorization: Bearer` header. The host adds no credentials of
    /// its own: the token is the app's.
    pub fn bearer(self, token: &str) -> Self {
        self.header("Authorization", format!("Bearer {token}"))
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Sends `value` as a JSON body, and asks for JSON back.
    pub fn json(self, value: &impl Serialize) -> Result<Self, serde_json::Error> {
        let body = serde_json::to_vec(value)?;
        Ok(self
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .with_body(body))
    }

    /// The first value of the header `name`, compared without case.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

impl ResponseHead {
    /// Whether the status is a success (2xx).
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The first value of the header `name`, compared without case.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}
