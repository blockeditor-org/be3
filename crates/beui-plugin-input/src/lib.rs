use block_plugin_api::{
    BackEdge, BackPhase, CursorIcon, FilePick, ImeInput, ImeText, Key, Modifiers, PointerButton,
    TouchPhase,
};

pub fn protocol_modifiers(modifiers: beui::Modifiers) -> Modifiers {
    Modifiers {
        alt: modifiers.alt,
        control: modifiers.ctrl,
        shift: modifiers.shift,
        command: modifiers.ctrl,
    }
}

pub fn pointer_button(button: beui::PointerButton) -> PointerButton {
    match button {
        beui::PointerButton::Primary => PointerButton::Primary,
        beui::PointerButton::Secondary => PointerButton::Secondary,
        beui::PointerButton::Middle => PointerButton::Middle,
        beui::PointerButton::Back => PointerButton::Back,
        beui::PointerButton::Forward => PointerButton::Forward,
    }
}

pub fn touch_phase(phase: beui::TouchPhase) -> TouchPhase {
    match phase {
        beui::TouchPhase::Start => TouchPhase::Start,
        beui::TouchPhase::Move => TouchPhase::Move,
        beui::TouchPhase::End => TouchPhase::End,
        beui::TouchPhase::Cancel => TouchPhase::Cancel,
    }
}

pub fn back_phase(gesture: beui::BackGesture) -> BackPhase {
    match gesture {
        beui::BackGesture::Started { edge } => BackPhase::Started {
            edge: match edge {
                beui::BackEdge::None => BackEdge::None,
                beui::BackEdge::Left => BackEdge::Left,
                beui::BackEdge::Right => BackEdge::Right,
            },
        },
        beui::BackGesture::Progressed(progress) => BackPhase::Progressed(progress),
        beui::BackGesture::Cancelled => BackPhase::Cancelled,
        beui::BackGesture::Invoked => BackPhase::Invoked,
    }
}

pub fn beui_back(phase: BackPhase) -> beui::BackGesture {
    match phase {
        BackPhase::Started { edge } => beui::BackGesture::Started {
            edge: match edge {
                BackEdge::None => beui::BackEdge::None,
                BackEdge::Left => beui::BackEdge::Left,
                BackEdge::Right => beui::BackEdge::Right,
            },
        },
        BackPhase::Progressed(progress) => beui::BackGesture::Progressed(progress),
        BackPhase::Cancelled => beui::BackGesture::Cancelled,
        BackPhase::Invoked => beui::BackGesture::Invoked,
    }
}

pub fn protocol_key(key: beui::Key) -> Key {
    match key {
        beui::Key::ArrowDown => Key::ArrowDown,
        beui::Key::ArrowLeft => Key::ArrowLeft,
        beui::Key::ArrowRight => Key::ArrowRight,
        beui::Key::ArrowUp => Key::ArrowUp,
        beui::Key::Backspace => Key::Backspace,
        beui::Key::BracketLeft => Key::OpenBracket,
        beui::Key::BracketRight => Key::CloseBracket,
        beui::Key::Delete => Key::Delete,
        beui::Key::End => Key::End,
        beui::Key::Enter => Key::Enter,
        beui::Key::Escape => Key::Escape,
        beui::Key::Home => Key::Home,
        beui::Key::Minus => Key::Minus,
        beui::Key::PageDown => Key::PageDown,
        beui::Key::PageUp => Key::PageUp,
        beui::Key::Plus => Key::Plus,
        beui::Key::Space => Key::Space,
        beui::Key::Tab => Key::Tab,
        beui::Key::Zero => Key::Num0,
        beui::Key::One => Key::Num1,
        beui::Key::Two => Key::Num2,
        beui::Key::Three => Key::Num3,
        beui::Key::Four => Key::Num4,
        beui::Key::Five => Key::Num5,
        beui::Key::Six => Key::Num6,
        beui::Key::Seven => Key::Num7,
        beui::Key::Eight => Key::Num8,
        beui::Key::Nine => Key::Num9,
        beui::Key::Backtick => Key::Backtick,
        beui::Key::Insert => Key::Insert,
        beui::Key::Comma => Key::Comma,
        beui::Key::Period => Key::Period,
        beui::Key::Slash => Key::Slash,
        beui::Key::Backslash => Key::Backslash,
        beui::Key::Semicolon => Key::Semicolon,
        beui::Key::Quote => Key::Quote,
        beui::Key::BrowserBack => Key::BrowserBack,
        beui::Key::A => Key::A,
        beui::Key::B => Key::B,
        beui::Key::C => Key::C,
        beui::Key::D => Key::D,
        beui::Key::E => Key::E,
        beui::Key::F => Key::F,
        beui::Key::G => Key::G,
        beui::Key::H => Key::H,
        beui::Key::I => Key::I,
        beui::Key::J => Key::J,
        beui::Key::K => Key::K,
        beui::Key::L => Key::L,
        beui::Key::M => Key::M,
        beui::Key::N => Key::N,
        beui::Key::O => Key::O,
        beui::Key::P => Key::P,
        beui::Key::Q => Key::Q,
        beui::Key::R => Key::R,
        beui::Key::S => Key::S,
        beui::Key::T => Key::T,
        beui::Key::U => Key::U,
        beui::Key::V => Key::V,
        beui::Key::W => Key::W,
        beui::Key::X => Key::X,
        beui::Key::Y => Key::Y,
        beui::Key::Z => Key::Z,
        beui::Key::F1 => Key::F1,
        beui::Key::F2 => Key::F2,
        beui::Key::F3 => Key::F3,
        beui::Key::F4 => Key::F4,
        beui::Key::F5 => Key::F5,
        beui::Key::F6 => Key::F6,
        beui::Key::F7 => Key::F7,
        beui::Key::F8 => Key::F8,
        beui::Key::F9 => Key::F9,
        beui::Key::F10 => Key::F10,
        beui::Key::F11 => Key::F11,
        beui::Key::F12 => Key::F12,
        beui::Key::F13 => Key::F13,
        beui::Key::F14 => Key::F14,
        beui::Key::F15 => Key::F15,
        beui::Key::F16 => Key::F16,
        beui::Key::F17 => Key::F17,
        beui::Key::F18 => Key::F18,
        beui::Key::F19 => Key::F19,
        beui::Key::F20 => Key::F20,
        beui::Key::F21 => Key::F21,
        beui::Key::F22 => Key::F22,
        beui::Key::F23 => Key::F23,
        beui::Key::F24 => Key::F24,
    }
}

pub fn beui_key(key: Key) -> Option<beui::Key> {
    let key = match key {
        Key::ArrowDown => beui::Key::ArrowDown,
        Key::ArrowLeft => beui::Key::ArrowLeft,
        Key::ArrowRight => beui::Key::ArrowRight,
        Key::ArrowUp => beui::Key::ArrowUp,
        Key::Backspace => beui::Key::Backspace,
        Key::OpenBracket => beui::Key::BracketLeft,
        Key::CloseBracket => beui::Key::BracketRight,
        Key::Delete => beui::Key::Delete,
        Key::End => beui::Key::End,
        Key::Enter => beui::Key::Enter,
        Key::Escape => beui::Key::Escape,
        Key::Home => beui::Key::Home,
        Key::Minus => beui::Key::Minus,
        Key::PageDown => beui::Key::PageDown,
        Key::PageUp => beui::Key::PageUp,
        Key::Plus | Key::Equals => beui::Key::Plus,
        Key::Space => beui::Key::Space,
        Key::Tab => beui::Key::Tab,
        Key::Num0 => beui::Key::Zero,
        Key::Num1 => beui::Key::One,
        Key::Num2 => beui::Key::Two,
        Key::Num3 => beui::Key::Three,
        Key::Num4 => beui::Key::Four,
        Key::Num5 => beui::Key::Five,
        Key::Num6 => beui::Key::Six,
        Key::Num7 => beui::Key::Seven,
        Key::Num8 => beui::Key::Eight,
        Key::Num9 => beui::Key::Nine,
        Key::Backtick => beui::Key::Backtick,
        Key::Insert => beui::Key::Insert,
        Key::Comma => beui::Key::Comma,
        Key::Period => beui::Key::Period,
        Key::Slash => beui::Key::Slash,
        Key::Backslash => beui::Key::Backslash,
        Key::Semicolon => beui::Key::Semicolon,
        Key::Quote => beui::Key::Quote,
        Key::BrowserBack => beui::Key::BrowserBack,
        Key::A => beui::Key::A,
        Key::B => beui::Key::B,
        Key::C => beui::Key::C,
        Key::D => beui::Key::D,
        Key::E => beui::Key::E,
        Key::F => beui::Key::F,
        Key::G => beui::Key::G,
        Key::H => beui::Key::H,
        Key::I => beui::Key::I,
        Key::J => beui::Key::J,
        Key::K => beui::Key::K,
        Key::L => beui::Key::L,
        Key::M => beui::Key::M,
        Key::N => beui::Key::N,
        Key::O => beui::Key::O,
        Key::P => beui::Key::P,
        Key::Q => beui::Key::Q,
        Key::R => beui::Key::R,
        Key::S => beui::Key::S,
        Key::T => beui::Key::T,
        Key::U => beui::Key::U,
        Key::V => beui::Key::V,
        Key::W => beui::Key::W,
        Key::X => beui::Key::X,
        Key::Y => beui::Key::Y,
        Key::Z => beui::Key::Z,
        Key::F1 => beui::Key::F1,
        Key::F2 => beui::Key::F2,
        Key::F3 => beui::Key::F3,
        Key::F4 => beui::Key::F4,
        Key::F5 => beui::Key::F5,
        Key::F6 => beui::Key::F6,
        Key::F7 => beui::Key::F7,
        Key::F8 => beui::Key::F8,
        Key::F9 => beui::Key::F9,
        Key::F10 => beui::Key::F10,
        Key::F11 => beui::Key::F11,
        Key::F12 => beui::Key::F12,
        Key::F13 => beui::Key::F13,
        Key::F14 => beui::Key::F14,
        Key::F15 => beui::Key::F15,
        Key::F16 => beui::Key::F16,
        Key::F17 => beui::Key::F17,
        Key::F18 => beui::Key::F18,
        Key::F19 => beui::Key::F19,
        Key::F20 => beui::Key::F20,
        Key::F21 => beui::Key::F21,
        Key::F22 => beui::Key::F22,
        Key::F23 => beui::Key::F23,
        Key::F24 => beui::Key::F24,
        _ => return None,
    };
    Some(key)
}

pub fn protocol_ime(event: &beui::ImeEvent) -> ImeInput {
    match event {
        beui::ImeEvent::Enabled => ImeInput::Enabled,
        beui::ImeEvent::Disabled => ImeInput::Disabled,
        beui::ImeEvent::SetComposingText(text) => ImeInput::SetComposingText(text.clone()),
        beui::ImeEvent::CommitText(text) => ImeInput::CommitText(text.clone()),
        beui::ImeEvent::FinishComposing => ImeInput::FinishComposing,
        beui::ImeEvent::SetComposingRegion(range) => ImeInput::SetComposingRegion {
            start: range.start as u64,
            end: range.end as u64,
        },
        beui::ImeEvent::ReplaceText { range, text } => ImeInput::ReplaceText {
            start: range.start as u64,
            end: range.end as u64,
            text: text.clone(),
        },
        beui::ImeEvent::DeleteSurrounding { before, after } => ImeInput::DeleteSurrounding {
            before: *before as u64,
            after: *after as u64,
        },
        beui::ImeEvent::SetSelection { anchor, focus } => ImeInput::SetSelection {
            anchor: *anchor as u64,
            focus: *focus as u64,
        },
    }
}

pub fn beui_ime(input: &ImeInput) -> beui::ImeEvent {
    match input {
        ImeInput::Enabled => beui::ImeEvent::Enabled,
        ImeInput::Disabled => beui::ImeEvent::Disabled,
        ImeInput::SetComposingText(text) => beui::ImeEvent::SetComposingText(text.clone()),
        ImeInput::CommitText(text) => beui::ImeEvent::CommitText(text.clone()),
        ImeInput::FinishComposing => beui::ImeEvent::FinishComposing,
        ImeInput::SetComposingRegion { start, end } => {
            beui::ImeEvent::SetComposingRegion(index(*start)..index(*end))
        }
        ImeInput::ReplaceText { start, end, text } => beui::ImeEvent::ReplaceText {
            range: index(*start)..index(*end),
            text: text.clone(),
        },
        ImeInput::DeleteSurrounding { before, after } => beui::ImeEvent::DeleteSurrounding {
            before: index(*before),
            after: index(*after),
        },
        ImeInput::SetSelection { anchor, focus } => beui::ImeEvent::SetSelection {
            anchor: index(*anchor),
            focus: index(*focus),
        },
    }
}

pub fn protocol_ime_text(text: &beui::ImeText) -> ImeText {
    ImeText {
        start: text.start as u64,
        text: text.text.clone(),
        selection: (text.selection.start as u64, text.selection.end as u64),
        composing: text
            .composing
            .as_ref()
            .map(|range| (range.start as u64, range.end as u64)),
    }
}

pub fn beui_ime_text(text: &ImeText) -> beui::ImeText {
    beui::ImeText {
        start: index(text.start),
        text: text.text.clone(),
        selection: index(text.selection.0)..index(text.selection.1),
        composing: text.composing.map(|(start, end)| index(start)..index(end)),
    }
}

fn index(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

pub fn beui_button(button: PointerButton) -> Option<beui::PointerButton> {
    match button {
        PointerButton::Primary => Some(beui::PointerButton::Primary),
        PointerButton::Secondary => Some(beui::PointerButton::Secondary),
        PointerButton::Middle => Some(beui::PointerButton::Middle),
        PointerButton::Back | PointerButton::Forward | PointerButton::Other(_) => None,
    }
}

pub fn beui_touch_phase(phase: TouchPhase) -> beui::TouchPhase {
    match phase {
        TouchPhase::Start => beui::TouchPhase::Start,
        TouchPhase::Move => beui::TouchPhase::Move,
        TouchPhase::End => beui::TouchPhase::End,
        TouchPhase::Cancel => beui::TouchPhase::Cancel,
    }
}

pub fn protocol_cursor(cursor: beui::CursorIcon) -> CursorIcon {
    match cursor {
        beui::CursorIcon::Default => CursorIcon::Default,
        beui::CursorIcon::Crosshair => CursorIcon::Crosshair,
        beui::CursorIcon::Grab => CursorIcon::Grab,
        beui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        beui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
        beui::CursorIcon::PointingHand => CursorIcon::Pointer,
        beui::CursorIcon::ResizeHorizontal => CursorIcon::ResizeHorizontal,
        beui::CursorIcon::ResizeVertical => CursorIcon::ResizeVertical,
        beui::CursorIcon::ResizeNeSw => CursorIcon::ResizeNeSw,
        beui::CursorIcon::ResizeNwSe => CursorIcon::ResizeNwSe,
        beui::CursorIcon::Text => CursorIcon::Text,
        beui::CursorIcon::Wait => CursorIcon::Wait,
        beui::CursorIcon::None => CursorIcon::None,
        beui::CursorIcon::Move => CursorIcon::Move,
        beui::CursorIcon::Progress => CursorIcon::Progress,
        beui::CursorIcon::Help => CursorIcon::Help,
        beui::CursorIcon::Alias => CursorIcon::Pointer,
    }
}

pub fn beui_file_pick(pick: FilePick) -> beui::FilePick {
    match pick {
        FilePick::Chosen { name, data } => Ok(Some(beui::PickedFile { name, data })),
        FilePick::Cancelled => Ok(None),
        FilePick::Failed(error) => Err(error),
    }
}
