use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Interactive, IntoProp, List, Memo, Prop, ReadSignal, Render, Show, clone,
    component_accessibility, create_effect, create_memo, create_signal, set_component_state,
};

const DRAG_THRESHOLD: f32 = 2.0;

#[derive(Clone, Copy, PartialEq)]
pub enum NumberDrag {
    Off,
    Linear { speed: f64 },
    Logarithmic { factor: f64 },
}

impl NumberDrag {
    fn applied(self, start: f64, points: f64) -> Option<f64> {
        match self {
            NumberDrag::Off => None,
            NumberDrag::Linear { speed } => Some(start + points * speed),
            NumberDrag::Logarithmic { factor } if factor > 1.0 => {
                let base = factor.ln();
                let exponent = start.max(f64::MIN_POSITIVE).ln() / base;
                Some(factor.powf(exponent + points))
            }
            NumberDrag::Logarithmic { .. } => None,
        }
    }
}

pub struct NumberFaceHandle {
    pub text: ReadSignal<String>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

pub struct NumberFieldHandle {
    pub text: ReadSignal<String>,
    pub editing: ReadSignal<bool>,
    pub on_change: Callback<String>,
    pub on_submit: Callback<String>,
    pub on_focus_change: Callback<bool>,
    pub on_key: Callback<KeyPress, bool>,
}

struct Parts {
    face: NodeId,
    field: NodeId,
    editing: ReadSignal<bool>,
}

#[component]
pub fn NumberInput(
    value: Prop<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = NumberDrag::Linear { speed: 1.0 })] drag: NumberDrag,
    face: Render<NumberFaceHandle>,
    field: Render<NumberFieldHandle>,
    on_change: Callback<f64>,
    on_preview: Callback<Option<f64>>,
) -> NodeId {
    let (text, set_text) = create_signal(format_number(value.peek()));
    let (editing, set_editing) = create_signal(false);
    let (refocus, set_refocus) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    create_effect(clone!(value text set_text -> move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse(&held) != Some(next) {
            set_text.set(format_number(next));
        }
    }));
    let off = create_memo(move || disabled.get());
    let idle = create_memo(clone!(editing -> move || !editing.get()));
    let cursor = create_memo(clone!(off -> move || {
        match drag != NumberDrag::Off && !off.get() {
            true => CursorIcon::ResizeHorizontal,
            false => CursorIcon::Default,
        }
    }));
    let accessibility = create_memo(clone!(text off -> move || {
        let mut node = Node::new(Role::SpinButton);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node.set_value(text.get());
        if off.get() {
            node.set_disabled();
        }
        node
    }));
    component_accessibility(accessibility);

    let held: Rc<Cell<Option<(Pos2, f64, bool)>>> = Rc::default();
    let pressed = clone!(held value off -> move |press: PointerPress| {
        if off.get_untracked() {
            return;
        }
        held.set(Some((press.pos, value.peek(), false)));
    });
    let changed = on_change.clone();
    let dragged = clone!(held set_text -> move |at: PointerPress| {
        let Some((origin, start, moved)) = held.get() else {
            return;
        };
        let points = at.pos.x - origin.x;
        if !moved && points.abs() < DRAG_THRESHOLD {
            return;
        }
        let Some(next) = drag.applied(start, f64::from(points)) else {
            return;
        };
        held.set(Some((origin, start, true)));
        let next = next.clamp(min, max);
        set_text.set(format_number(next));
        changed.call(next);
    });
    let open = clone!(off set_editing -> move || {
        if !off.get_untracked() {
            set_editing.set(true);
        }
    });
    let clicked = clone!(held open -> move || {
        if let Some((_, _, moved)) = held.take()
            && !moved
        {
            open();
        }
    });

    let previewed = on_preview.clone();
    let edited = clone!(set_text editing -> move |typed: String| {
        if !editing.get_untracked() {
            return;
        }
        set_text.set(typed.clone());
        previewed.call(parse(&typed).map(|parsed| parsed.clamp(min, max)));
    });
    let finish = clone!(value text set_text editing set_editing -> move |keep: bool| {
        if !editing.get_untracked() {
            return;
        }
        set_editing.set(false);
        let typed = parse(&text.get_untracked())
            .filter(|_| keep)
            .map(|typed| typed.clamp(min, max));
        let shown = value.peek();
        match typed {
            Some(typed) if typed != shown => {
                set_text.set(format_number(typed));
                on_change.call(typed);
            }
            _ => set_text.set(format_number(shown)),
        }
        on_preview.call(None);
    });
    let submitted = clone!(finish set_refocus -> move |_: String| {
        set_refocus.set(true);
        finish(true);
    });
    let focus_changed = clone!(finish -> move |focused: bool| {
        if !focused {
            finish(true);
        }
    });
    let escaped = clone!(set_refocus -> move |press: KeyPress| {
        if press.key != Key::Escape {
            return false;
        }
        set_refocus.set(true);
        finish(false);
        true
    });
    let button_focus = move |has_focus: bool| {
        set_focused.set(has_focus);
        if !has_focus {
            set_refocus.set(false);
        }
    };
    let tab_stop = off.clone().into_prop().map(|off: bool| !off);

    let face = face.call(NumberFaceHandle {
        text: text.clone(),
        hovered,
        active,
        focused,
        disabled: off,
    });
    let field = field.call(NumberFieldHandle {
        text,
        editing: editing.clone(),
        on_change: Callback::new(edited),
        on_submit: Callback::new(submitted),
        on_focus_change: Callback::new(focus_changed),
        on_key: Callback::new(escaped),
    });
    set_component_state(Parts {
        face,
        field,
        editing: editing.clone(),
    });
    view! {
        <List spacing=0.0>
            <Show condition={idle}>
                <Interactive
                    focusable=true
                    tab_stop
                    focused={refocus}
                    on_focus_change={button_focus}
                    on_activate={open.clone()}
                    cursor={cursor}
                    capture_presses=true
                    on_press={pressed}
                    on_drag={dragged}
                    on_click={clicked}
                    on_hover_change={move |is_hovered: bool| set_hovered.set(is_hovered)}
                    on_active_change={move |is_active: bool| set_active.set(is_active)}
                    children={Some(face)}
                />
            </Show>
            <Show condition={editing}>{field}</Show>
        </List>
    }
}

pub fn number_input_field(document: &Document, input: NodeId) -> Option<NodeId> {
    let parts = document.component_state::<Parts>(input);
    parts.editing.get_untracked().then_some(parts.field)
}

pub fn number_input_face(document: &Document, input: NodeId) -> Option<NodeId> {
    let parts = document.component_state::<Parts>(input);
    (!parts.editing.get_untracked()).then_some(parts.face)
}

fn parse(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    match trimmed.is_empty() {
        true => None,
        false => trimmed
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite()),
    }
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}
