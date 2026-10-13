use beui_core::clipboard::ImagePaste;
#[cfg(not(target_os = "ios"))]
use beui_core::file_picker::PickedFile;
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

    pub fn image(&mut self) -> ImagePaste {
        #[cfg(not(target_os = "ios"))]
        {
            let clipboard = match &mut self.inner {
                Some(clipboard) => clipboard,
                None => match arboard::Clipboard::new() {
                    Ok(clipboard) => self.inner.insert(clipboard),
                    Err(error) => return Err(error.to_string()),
                },
            };
            let image = match clipboard.get_image() {
                Ok(image) => image,
                Err(arboard::Error::ContentNotAvailable) => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            let mut data = Vec::new();
            PngEncoder::new(&mut data)
                .write_image(
                    image.bytes.as_ref(),
                    image.width as u32,
                    image.height as u32,
                    ExtendedColorType::Rgba8,
                )
                .map_err(|error| format!("Could not encode pasted image: {error}"))?;
            Ok(Some(PickedFile {
                name: PASTED_NAME.to_owned(),
                data,
            }))
        }
        #[cfg(target_os = "ios")]
        {
            Ok(None)
        }
    }
}
