//! QR codes for links a person shows another device to scan (an invite link), drawn by be-host
//! and its root alike.

use qrcodegen::{QrCode, QrCodeEcc};

/// The quiet zone around a code, in modules, as the standard asks.
const QUIET: i32 = 4;

/// A QR code's pixels: one per module with its quiet zone, dark on white, for a picture that
/// scales it up unsmoothed.
pub struct Code {
    /// Its width and height, in pixels.
    pub side: u32,
    /// Its RGBA, row by row.
    pub rgba: Vec<u8>,
}

/// `text` as a QR code. None for text too long for any code.
pub fn code(text: &str) -> Option<Code> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let side = code.size() + 2 * QUIET;
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let shade = match code.get_module(x - QUIET, y - QUIET) {
                true => 0,
                false => 255,
            };
            rgba.extend([shade, shade, shade, 255]);
        }
    }
    Some(Code {
        side: side as u32,
        rgba,
    })
}
