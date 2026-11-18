//! QR codes for links a person shows another device to scan (an invite link).

use beui::Image;
use qrcodegen::{QrCode, QrCodeEcc};

/// The quiet zone around a code, in modules, as the standard asks.
const QUIET: i32 = 4;

/// `text` as a QR code, one pixel per module with its quiet zone: dark on white, for a
/// `Picture` that scales it up unsmoothed. None for text too long for any code.
pub(crate) fn image(text: &str) -> Option<Image> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let side = code.size() + 2 * QUIET;
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let shade = match code.get_module(x - QUIET, y - QUIET) {
                true => 0,
                false => 255,
            };
            pixels.extend([shade, shade, shade, 255]);
        }
    }
    Some(Image::from_rgba(side as u32, side as u32, pixels))
}
