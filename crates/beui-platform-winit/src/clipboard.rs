use beui_types::PickedFile;
#[cfg(not(target_os = "ios"))]
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

#[cfg(not(target_os = "ios"))]
const PASTED_NAME: &str = "Pasted Image.png";

#[derive(Default)]
pub struct Clipboard {
    #[cfg(not(target_os = "ios"))]
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_os = "ios"))]
            inner: arboard::Clipboard::new().ok(),
        }
    }

    pub fn get(&mut self) -> Option<String> {
        #[cfg(not(target_os = "ios"))]
        {
            self.inner.as_mut()?.get_text().ok()
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
    }

    pub fn set(&mut self, text: String) {
        #[cfg(not(target_os = "ios"))]
        if let Some(clipboard) = &mut self.inner {
            let _ = clipboard.set_text(text);
        }
        #[cfg(target_os = "ios")]
        let _ = text;
    }

    pub fn set_image(&mut self, png: &[u8]) {
        #[cfg(not(target_os = "ios"))]
        {
            let Some(clipboard) = &mut self.inner else {
                return;
            };
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
        #[cfg(target_os = "ios")]
        let _ = png;
    }

    pub fn image(&mut self) -> Option<PickedFile> {
        #[cfg(not(target_os = "ios"))]
        {
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
            Some(PickedFile {
                name: PASTED_NAME.to_owned(),
                data,
            })
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
    }
}
