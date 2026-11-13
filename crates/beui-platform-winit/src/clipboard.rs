use beui_core::clipboard::{Clip, HeldClip, PasteAnswer};
use beui_types::ClipItem;
#[cfg(not(target_os = "ios"))]
use beui_types::clipboard::formats;
#[cfg(not(target_os = "ios"))]
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

#[cfg(all(
    unix,
    not(any(target_os = "android", target_os = "macos", target_os = "ios"))
))]
mod x11;

/// The system clipboard. A copy puts its plain text and its PNG there
/// together: on X11 (and Wayland, through XWayland) by owning the selection
/// itself (`x11`), on Windows as `CF_UNICODETEXT` beside `CF_DIB` and a
/// registered `PNG` format, elsewhere through arboard, which carries one of
/// them. Pastes from other programs read through arboard.
/// The clip a copy wrote stays held, so pastes in this program get its other
/// representations until another program copies: on Windows the clipboard's
/// sequence number says so, and be3 formats are registered formats there too;
/// elsewhere the text on the clipboard is compared with the text written.
#[derive(Default)]
pub struct Clipboard {
    #[cfg(not(target_os = "ios"))]
    inner: Option<arboard::Clipboard>,
    #[cfg(all(
        unix,
        not(any(target_os = "android", target_os = "macos", target_os = "ios"))
    ))]
    x11: Option<x11::Owner>,
    held: HeldClip,
    #[cfg(target_os = "windows")]
    sequence: Option<u32>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_os = "ios"))]
            inner: arboard::Clipboard::new().ok(),
            #[cfg(all(
                unix,
                not(any(target_os = "android", target_os = "macos", target_os = "ios"))
            ))]
            x11: x11::Owner::connect(None),
            ..Self::default()
        }
    }

    pub fn copy(&mut self, clip: Clip) {
        #[cfg(not(target_os = "ios"))]
        self.write(&clip);
        #[cfg(target_os = "windows")]
        {
            windows::write(&clip.ready_items(), clip.plain_text().is_some());
            self.sequence = windows::sequence();
        }
        self.held.copy(clip);
    }

    pub fn paste(&mut self, wanted: &[String], answer: PasteAnswer) {
        let ours = self.ours();
        if let Some(clip) = self.held.current(ours) {
            return clip.resolve(wanted, move |item| answer.send(item));
        }
        let offered = self.offered_be3(wanted);
        answer.send(
            wanted
                .iter()
                .find_map(|format| self.read(format, offered.as_deref())),
        );
    }

    fn ours(&mut self) -> bool {
        if !self.held.holds() {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            windows::sequence() == self.sequence
        }
        #[cfg(not(target_os = "windows"))]
        {
            let now = self.text();
            self.held.wrote(now.as_deref())
        }
    }

    #[cfg(not(target_os = "ios"))]
    fn write(&mut self, clip: &Clip) {
        #[cfg(all(
            unix,
            not(any(target_os = "android", target_os = "macos", target_os = "ios"))
        ))]
        if let Some(owner) = &self.x11 {
            return owner.offer(&clip.ready_items());
        }
        let Some(clipboard) = &mut self.inner else {
            return;
        };
        if let Some(text) = clip.plain_text() {
            let _ = clipboard.set_text(text);
        } else if let Some(png) = clip.ready(formats::PNG) {
            set_image(clipboard, png);
        } else {
            let _ = clipboard.clear();
        }
    }

    /// On X11, the targets the selection's owner offers, asked once a paste
    /// and only when it wants a be3 format, so each be3 format read is one
    /// the owner has.
    fn offered_be3(&self, wanted: &[String]) -> Option<Vec<String>> {
        #[cfg(all(
            unix,
            not(any(target_os = "android", target_os = "macos", target_os = "ios"))
        ))]
        if wanted.iter().any(|format| formats::is_be3(format)) {
            return Some(self.x11.as_ref()?.targets());
        }
        let _ = wanted;
        None
    }

    fn read(&mut self, format: &str, offered: Option<&[String]>) -> Option<ClipItem> {
        let _ = offered;
        #[cfg(not(target_os = "ios"))]
        {
            match format {
                formats::TEXT => self.text().map(ClipItem::text),
                formats::PNG => self.image().map(ClipItem::png),
                #[cfg(target_os = "windows")]
                format if formats::is_be3(format) => windows::read(format),
                #[cfg(all(
                    unix,
                    not(any(target_os = "android", target_os = "macos", target_os = "ios"))
                ))]
                format
                    if formats::is_be3(format)
                        && offered.is_some_and(|offered| offered.iter().any(|held| held == format)) =>
                {
                    self.x11.as_ref()?.read(format)
                }
                _ => None,
            }
        }
        #[cfg(target_os = "ios")]
        {
            let _ = format;
            None
        }
    }

    fn text(&mut self) -> Option<String> {
        #[cfg(not(target_os = "ios"))]
        {
            self.inner
                .as_mut()?
                .get_text()
                .ok()
                .filter(|text| !text.is_empty())
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
    }

    #[cfg(not(target_os = "ios"))]
    fn image(&mut self) -> Option<Vec<u8>> {
        let image = match self.inner.as_mut()?.get_image() {
            Ok(image) => image,
            Err(arboard::Error::ContentNotAvailable) => return None,
            Err(error) => {
                eprintln!("beui: could not read the clipboard's image: {error}");
                return None;
            }
        };
        let mut data = Vec::new();
        if let Err(error) = PngEncoder::new(&mut data).write_image(
            image.bytes.as_ref(),
            image.width as u32,
            image.height as u32,
            ExtendedColorType::Rgba8,
        ) {
            eprintln!("beui: could not encode the clipboard's image: {error}");
            return None;
        }
        Some(data)
    }
}

#[cfg(not(target_os = "ios"))]
fn set_image(clipboard: &mut arboard::Clipboard, png: &[u8]) {
    let image = match image::load_from_memory_with_format(png, image::ImageFormat::Png) {
        Ok(image) => image.into_rgba8(),
        Err(error) => {
            eprintln!("beui: could not decode an image to copy: {error}");
            return;
        }
    };
    let (width, height) = image.dimensions();
    if let Err(error) = clipboard.set_image(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: image.into_raw().into(),
    }) {
        eprintln!("beui: could not copy an image: {error}");
    }
}

/// be3 formats as registered clipboard formats, named by their format id and
/// holding their `stored` bytes, refs and all, so another be3 program reads
/// them; a PNG beside the text arboard wrote, as `CF_DIB` and the registered
/// `PNG` format other programs read; and the sequence number that changes with
/// every copy.
#[cfg(target_os = "windows")]
mod windows {
    use beui_types::ClipItem;
    use beui_types::clipboard::formats;

    const CF_DIB: u32 = 8;

    pub(super) fn sequence() -> Option<u32> {
        clipboard_win::raw::seq_num().map(u32::from)
    }

    /// Adds to what arboard wrote: be3 formats, and the PNG when arboard
    /// wrote the `text` instead of it.
    pub(super) fn write(items: &[ClipItem], text: bool) {
        let be3 = items
            .iter()
            .filter(|item| formats::is_be3(&item.format))
            .collect::<Vec<_>>();
        let png = items
            .iter()
            .find(|item| item.format == formats::PNG)
            .filter(|_| text);
        if be3.is_empty() && png.is_none() {
            return;
        }
        let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else {
            return;
        };
        for item in be3 {
            if let Some(format) = clipboard_win::register_format(&item.format) {
                let _ = clipboard_win::raw::set_without_clear(format.get(), &item.stored());
            }
        }
        if let Some(png) = png {
            if let Some(format) = clipboard_win::register_format("PNG") {
                let _ = clipboard_win::raw::set_without_clear(format.get(), &png.data);
            }
            if let Some(dib) = super::dib(&png.data) {
                let _ = clipboard_win::raw::set_without_clear(CF_DIB, &dib);
            }
        }
    }

    pub(super) fn read(format: &str) -> Option<ClipItem> {
        let id = clipboard_win::register_format(format)?.get();
        let _open = clipboard_win::Clipboard::new_attempts(10).ok()?;
        if !clipboard_win::raw::is_format_avail(id) {
            return None;
        }
        let mut data = Vec::new();
        clipboard_win::raw::get_vec(id, &mut data).ok()?;
        ClipItem::from_stored(format, &data)
    }
}

/// A PNG as a device-independent bitmap: a `BITMAPINFOHEADER` and 32-bit BGRA
/// rows, bottom row first, as Windows' `CF_DIB` holds an image.
#[cfg(any(target_os = "windows", test))]
fn dib(png: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let mut dib = Vec::with_capacity(40 + image.as_raw().len());
    dib.extend(40u32.to_le_bytes());
    dib.extend(i32::try_from(width).ok()?.to_le_bytes());
    dib.extend(i32::try_from(height).ok()?.to_le_bytes());
    dib.extend(1u16.to_le_bytes());
    dib.extend(32u16.to_le_bytes());
    dib.extend(0u32.to_le_bytes()); // BI_RGB
    dib.extend(u32::try_from(image.as_raw().len()).ok()?.to_le_bytes());
    dib.extend([0u8; 16]); // resolution and palette
    for row in image.rows().rev() {
        for pixel in row {
            let [red, green, blue, alpha] = pixel.0;
            dib.extend([blue, green, red, alpha]);
        }
    }
    Some(dib)
}

#[cfg(test)]
mod tests;
