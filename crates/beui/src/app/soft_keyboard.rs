use winit::platform::android::activity::AndroidApp;
use winit::platform::android::activity::input::{
    ImeOptions, InputType, TextInputAction, TextInputState,
};

use crate::input::Event;

const CLEAR_AFTER: usize = 1024;

pub struct SoftKeyboard {
    mirrored: Option<String>,
    clearing: Option<String>,
}

impl SoftKeyboard {
    pub fn new() -> Self {
        Self {
            mirrored: None,
            clearing: None,
        }
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
        let state = app.text_input_state();
        if let Some(cleared) = &self.clearing
            && !state.text.starts_with(cleared.as_str())
        {
            self.clearing = None;
            mirrored.clear();
        }
        if state.text != *mirrored {
            super::typed(mirrored, &state.text, events);
            *mirrored = state.text;
        }
        let composing = state
            .compose_region
            .is_some_and(|region| region.start != region.end);
        if self.clearing.is_none() && mirrored.len() > CLEAR_AFTER && !composing {
            self.clearing = Some(mirrored.clone());
            app.set_text_input_state(TextInputState::default());
        }
    }
}
