use beui_types::input::Input;

pub fn type_events(events: Vec<Input>) {
    #[cfg(target_arch = "wasm32")]
    web::type_events(events);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = events;
}

pub fn request_typed_paste() {
    #[cfg(target_arch = "wasm32")]
    web::request_typed_paste();
}

pub fn toggle_keyboard() {
    #[cfg(target_arch = "wasm32")]
    web::toggle_keyboard();
}

pub fn close_keyboard() {
    crate::ime::show_keyboard(false);
    #[cfg(target_arch = "wasm32")]
    web::SHOWN.with(|shown| shown.set(false));
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::Cell;

    use beui_types::input::{Input, Key, Keystroke, Modifiers, typing};
    use wasm_bindgen::JsCast;

    thread_local! {
        static HELD: Cell<Modifiers> = const { Cell::new(Modifiers::NONE) };
        pub(super) static SHOWN: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn type_events(events: Vec<Input>) {
        for event in events {
            match event {
                Input::Modifiers(modifiers) => HELD.with(|held| held.set(modifiers)),
                Input::Key(stroke) => key(stroke),
                Input::Text(text) => crate::ime::commit(text),
                _ => {}
            }
        }
        reactive::flush();
    }

    fn value(key: Key, text: Option<&str>) -> Option<String> {
        if let Some(text) = text {
            return Some(text.to_owned());
        }
        let named = match key {
            Key::ArrowDown => "ArrowDown",
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::ArrowUp => "ArrowUp",
            Key::Backspace => "Backspace",
            Key::Delete => "Delete",
            Key::End => "End",
            Key::Enter => "Enter",
            Key::Escape => "Escape",
            Key::Home => "Home",
            Key::PageDown => "PageDown",
            Key::PageUp => "PageUp",
            Key::Tab => "Tab",
            Key::Insert => "Insert",
            Key::Shift => "Shift",
            Key::Ctrl => "Control",
            Key::Alt => "Alt",
            Key::Logo => "Meta",
            Key::Space => " ",
            key => {
                return r"abcdefghijklmnopqrstuvwxyz0123456789-=,./\;'`[]"
                    .chars()
                    .find(|character| Key::unshifted(*character) == Some(key))
                    .map(String::from);
            }
        };
        Some(named.to_owned())
    }

    fn key(stroke: Keystroke) {
        let Some(key) = stroke.key else {
            return;
        };
        let Some(value) = value(key, stroke.text.as_deref()) else {
            return;
        };
        let Some(document) = crate::dom::document() else {
            return;
        };
        let target: web_sys::EventTarget = match document.active_element() {
            Some(active) => active.into(),
            None => match document.body() {
                Some(body) => body.into(),
                None => return,
            },
        };
        let held = HELD.with(Cell::get);
        let init = web_sys::KeyboardEventInit::new();
        init.set_key(&value);
        init.set_bubbles(true);
        init.set_cancelable(true);
        init.set_repeat(stroke.repeat);
        init.set_shift_key(held.shift);
        init.set_ctrl_key(held.ctrl);
        init.set_alt_key(held.alt);
        init.set_meta_key(held.logo);
        let name = if stroke.pressed { "keydown" } else { "keyup" };
        let Ok(event) = web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(name, &init)
        else {
            return;
        };
        let taken = !target.dispatch_event(&event).unwrap_or(true);
        if !stroke.pressed || taken {
            return;
        }
        if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
            edit(input, key, stroke.text.as_deref());
        } else if let Some(text) = stroke.text {
            crate::ime::commit(text);
        }
    }

    fn edit(input: &web_sys::HtmlInputElement, key: Key, text: Option<&str>) {
        let length = input.value().encode_utf16().count() as u32;
        let start = input.selection_start().ok().flatten().unwrap_or(length);
        let end = input.selection_end().ok().flatten().unwrap_or(start);
        let collapsed = start == end;
        let replace = |text: &str, from: u32, to: u32| {
            let _ = input.set_range_text_with_start_and_end_and_selection_mode(
                text,
                from,
                to,
                web_sys::SelectionMode::End,
            );
            let init = web_sys::EventInit::new();
            init.set_bubbles(true);
            if let Ok(event) = web_sys::Event::new_with_event_init_dict("input", &init) {
                let _ = input.dispatch_event(&event);
            }
        };
        let place = |at: u32| {
            let _ = input.set_selection_range(at, at);
        };
        match (key, text) {
            (Key::Enter, _) => {
                if let Some(form) = input.form() {
                    let _ = form.request_submit();
                }
            }
            (_, Some(text)) => replace(text, start, end),
            (Key::Backspace, _) if !collapsed => replace("", start, end),
            (Key::Backspace, _) if start > 0 => replace("", start - 1, start),
            (Key::Delete, _) if !collapsed => replace("", start, end),
            (Key::Delete, _) if end < length => replace("", start, end + 1),
            (Key::ArrowLeft, _) => place(match collapsed {
                true => start.saturating_sub(1),
                false => start,
            }),
            (Key::ArrowRight, _) => place(match collapsed {
                true => (end + 1).min(length),
                false => end,
            }),
            (Key::Home, _) => place(0),
            (Key::End, _) => place(length),
            _ => {}
        }
    }

    pub(super) fn request_typed_paste() {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let promise = window.navigator().clipboard().read_text();
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(text) = wasm_bindgen_futures::JsFuture::from(promise).await else {
                return;
            };
            let Some(text) = text.as_string() else {
                return;
            };
            let mut events = Vec::new();
            typing::typed(&text, HELD.with(Cell::get), &mut events);
            type_events(events);
        });
    }

    pub(super) fn toggle_keyboard() {
        let shown = !SHOWN.with(Cell::get);
        SHOWN.with(|held| held.set(shown));
        crate::ime::show_keyboard(shown);
    }
}
