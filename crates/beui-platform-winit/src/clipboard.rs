use beui_core::clipboard::{Clip, HeldClip, PasteAnswer};
use beui_types::ClipItem;
#[cfg(not(target_os = "ios"))]
use beui_types::clipboard::formats;
#[cfg(not(target_os = "ios"))]
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

/// The system clipboard through arboard, which carries plain text and images.
/// The clip a copy wrote stays held, so pastes in this program get its other
/// representations until another program copies: on Windows the clipboard's
/// sequence number says so, and be3 formats are registered formats there too;
/// elsewhere the text on the clipboard is compared with the text written.
#[derive(Default)]
pub struct Clipboard {
    #[cfg(not(target_os = "ios"))]
    inner: Option<arboard::Clipboard>,
    held: HeldClip,
    #[cfg(target_os = "windows")]
    sequence: Option<u32>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_os = "ios"))]
            inner: arboard::Clipboard::new().ok(),
            ..Self::default()
        }
    }

    pub fn copy(&mut self, clip: Clip) {
        #[cfg(not(target_os = "ios"))]
        self.write(&clip);
        #[cfg(target_os = "windows")]
        {
            windows::write(&clip.ready_items());
            self.sequence = windows::sequence();
        }
        self.held.copy(clip);
    }

    pub fn paste(&mut self, wanted: &[String], answer: PasteAnswer) {
        let ours = self.ours();
        if let Some(clip) = self.held.current(ours) {
            return clip.resolve(wanted, move |item| answer.send(item));
        }
        answer.send(wanted.iter().find_map(|format| self.read(format)));
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

    fn read(&mut self, format: &str) -> Option<ClipItem> {
        #[cfg(not(target_os = "ios"))]
        {
            match format {
                formats::TEXT => self.text().map(ClipItem::text),
                formats::PNG => self.image().map(ClipItem::png),
                #[cfg(target_os = "windows")]
                format if formats::is_be3(format) => windows::read(format),
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

/// be3 formats as registered clipboard formats, named by their format id, so
/// another be3 program reads them; and the sequence number that changes with
/// every copy.
#[cfg(target_os = "windows")]
mod windows {
    use beui_types::ClipItem;
    use beui_types::clipboard::formats;

    pub(super) fn sequence() -> Option<u32> {
        clipboard_win::raw::seq_num().map(u32::from)
    }

    pub(super) fn write(items: &[ClipItem]) {
        let be3 = items
            .iter()
            .filter(|item| formats::is_be3(&item.format))
            .collect::<Vec<_>>();
        if be3.is_empty() {
            return;
        }
        let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else {
            return;
        };
        for item in be3 {
            if let Some(format) = clipboard_win::register_format(&item.format) {
                let _ = clipboard_win::raw::set_without_clear(format.get(), &item.data);
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
        Some(ClipItem::new(format, data))
    }
}
