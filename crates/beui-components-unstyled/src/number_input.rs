use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::tree::component_node;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::reactive::{KeyAt, KeyEvent};
use beui_view::types::KeyPhase;
use beui_view::types::Pos2;
use beui_view::types::{CursorIcon, Key, PointerPress};

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
    pub text: Binding<String>,
    pub editing: ReadSignal<bool>,
    pub on_submit: Callback<String>,
    pub on_focus_change: Callback<bool>,
}

#[derive(Clone, Copy)]
struct Parts {
    face: NodeId,
    field: NodeId,
    editing: ReadSignal<bool>,
}

#[component]
pub fn NumberInput(
    #[prop(default = ComponentRef::new())] api: ComponentRef<NumberInputApi>,
    value: Binding<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = NumberDrag::Linear { speed: 1.0 })] drag: NumberDrag,
    #[prop(default = None)] decimals: Prop<Option<u32>>,
    face: Render<NumberFaceHandle>,
    field: Render<NumberFieldHandle>,
    on_preview: Callback<Option<f64>>,
    on_scrub: Callback<bool>,
) -> NodeId {
    let text = create_signal(format_number(
        value.get_untracked(),
        decimals.get_untracked(),
    ));
    let editing = create_signal(false);
    let refocus = create_signal(false);
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    create_effect(move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse(&held) != Some(next) {
            text.set(format_number(next, decimals.get_untracked()));
        }
    });
    let off = create_memo(move || disabled.get());
    let idle = create_memo(move || !editing.get());
    let cursor = create_memo(move || match drag != NumberDrag::Off && !off.get() {
        true => CursorIcon::ResizeHorizontal,
        false => CursorIcon::Default,
    });
    let accessibility = create_memo(move || {
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
    });
    component_accessibility(accessibility);

    let held = StoredCell::new(None::<(Pos2, f64, bool)>);
    let pressed = move |press: PointerPress| {
        if off.get_untracked() {
            return;
        }
        held.set(Some((press.pos, value.get_untracked(), false)));
    };
    let dragged = move |at: PointerPress| {
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
        if !moved {
            on_scrub.call(true);
        }
        let next = next.clamp(min, max);
        text.set(format_number(next, decimals.get_untracked()));
        value.set(next);
    };
    let open = move || {
        if !off.get_untracked() {
            editing.set(true);
        }
    };
    let clicked = move || match held.take() {
        Some((_, _, false)) => open(),
        Some((_, _, true)) => on_scrub.call(false),
        None => {}
    };
    let cancelled = move || {
        if let Some((_, _, true)) = held.take() {
            on_scrub.call(false);
        }
    };

    let edited = move |typed: String| {
        if !editing.get_untracked() {
            return;
        }
        text.set(typed.clone());
        on_preview.call(parse(&typed).map(|parsed| parsed.clamp(min, max)));
    };
    let finish = move |keep: bool| {
        if !editing.get_untracked() {
            return;
        }
        editing.set(false);
        let typed = parse(&text.get_untracked())
            .filter(|_| keep)
            .map(|typed| typed.clamp(min, max));
        match typed {
            Some(typed) => {
                text.set(format_number(typed, decimals.get_untracked()));
                value.update(|held| *held = typed);
            }
            None => text.set(format_number(
                value.get_untracked(),
                decimals.get_untracked(),
            )),
        }
        on_preview.call(None);
    };
    let submitted = move |_: String| {
        refocus.set(true);
        finish(true);
    };
    let focus_changed = move |focused: bool| {
        if !focused {
            finish(true);
        }
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        move |event: KeyEvent| {
            let escaped = event
                .press()
                .is_some_and(|press| press.pressed && press.key == Key::Escape);
            if !escaped || !editing.get_untracked() {
                return false;
            }
            refocus.set(true);
            finish(false);
            true
        },
    );
    let button_focus = move |has_focus: bool| {
        focused.set(has_focus);
        if !has_focus {
            refocus.set(false);
        }
    };
    let tab_stop = off.into_prop().map(|off: bool| !off);

    let face = face.call(NumberFaceHandle {
        text: text.read_only(),
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
        disabled: off,
    });
    let field = field.call(NumberFieldHandle {
        text: Binding::new(text, edited),
        editing: editing.read_only(),
        on_submit: Callback::new(submitted),
        on_focus_change: Callback::new(focus_changed),
    });
    api.fill(NumberInputApi(Parts {
        face,
        field,
        editing: editing.read_only(),
    }));
    view! {
        <List spacing=0.0>
            <ShowKeepAlive condition={idle}>
                <Interactive
                    focusable=true
                    tab_stop
                    focused={refocus}
                    on_focus_change={button_focus}
                    on_activate={open}
                    cursor={cursor}
                    capture_presses=true
                    on_press={pressed}
                    on_drag={dragged}
                    on_click={clicked}
                    on_cancel={cancelled}
                    on_hover_change={hovered}
                    on_active_change={active}
                    children={Some(face)}
                />
            </ShowKeepAlive>
            <ShowKeepAlive condition={editing}>{field}</ShowKeepAlive>
        </List>
    }
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

const MAX_DECIMALS: u32 = 15;

fn format_number(value: f64, decimals: Option<u32>) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let decimals = decimals
        .map(|decimals| decimals.min(MAX_DECIMALS))
        .filter(|decimals| (value * 10f64.powi(*decimals as i32)).is_finite());
    let value = match decimals {
        Some(decimals) => {
            let factor = 10f64.powi(decimals as i32);
            (value * factor).round() / factor + 0.0
        }
        None => value,
    };
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    match decimals {
        Some(decimals) => {
            let fixed = format!("{value:.0$}", decimals as usize);
            fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
        None => format!("{value}"),
    }
}

#[derive(Clone, Copy)]
pub struct NumberInputApi(Parts);

impl NumberInputApi {
    pub fn field(&self) -> Option<NodeId> {
        let parts = &self.0;
        parts.editing.get_untracked().then_some(parts.field)
    }

    pub fn face(&self) -> Option<NodeId> {
        let parts = &self.0;
        (!parts.editing.get_untracked()).then_some(parts.face)
    }
}
