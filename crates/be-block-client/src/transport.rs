#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{Reader, Writer, connect, post_once, request, spawn};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{Reader, Writer, connect, post_once, request, spawn};

pub(crate) enum Frame {
    Binary(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    Ping(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    Failed(String),
    Closed,
}

/// One frame as an HTTP post carries it: its length, then its bytes.
pub(crate) fn framed(frame: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(frame.len() + 4);
    body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
    body.extend_from_slice(frame);
    body
}

pub(crate) fn frames(body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut frames = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        if rest.len() < 4 {
            return Err("the server's answer ends partway through a frame".to_owned());
        }
        let size = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let frame = rest
            .get(4..4 + size)
            .ok_or_else(|| "the server's answer ends partway through a frame".to_owned())?;
        frames.push(frame.to_vec());
        rest = &rest[4 + size..];
    }
    Ok(frames)
}
