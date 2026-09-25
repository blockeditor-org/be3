use winit::platform::android::activity::AndroidApp;
use winit::platform::android::activity::input::{ImeOptions, InputType, TextInputAction};

use crate::input::Event;

pub struct SoftKeyboard {
    mirrored: Option<String>,
}

impl SoftKeyboard {
    pub fn new() -> Self {
        Self { mirrored: None }
    }

    pub fn show(&mut self, app: &AndroidApp) {
        app.set_ime_editor_info(
            InputType::TYPE_CLASS_TEXT
                | InputType::TYPE_TEXT_FLAG_MULTI_LINE
                | InputType::TYPE_TEXT_FLAG_AUTO_CORRECT,
            TextInputAction::None,
            ImeOptions::IME_FLAG_NO_FULLSCREEN,
        );
        self.mirrored = Some(app.text_input_state().text);
        app.show_soft_input(false);
    }

    pub fn hide(&mut self, app: &AndroidApp) {
        self.mirrored = None;
        app.hide_soft_input(false);
    }

    pub fn read(&mut self, app: &AndroidApp, events: &mut Vec<Event>) {
        let Some(mirrored) = &mut self.mirrored else {
            return;
        };
        let text = app.text_input_state().text;
        if text == *mirrored {
            return;
        }
        super::typed(mirrored, &text, events);
        *mirrored = text;
    }
}
