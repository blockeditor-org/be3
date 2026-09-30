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
}
