use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Focusable, Frame, IntoProp, List, Memo, NodeRef, Prop, ReadSignal,
    Show, Text, clone, component_accessibility, create_effect, create_memo, create_signal,
    set_component_state,
};
use crate::styled::text_input::TextInput;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, use_theme};

const DRAG_THRESHOLD: f32 = 2.0;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

struct Field {
    field: NodeRef,
    face: NodeRef,
    editing: ReadSignal<bool>,
}

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

#[component]
pub fn NumberInput(
    value: Prop<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = NumberDrag::Linear { speed: 1.0 })] drag: NumberDrag,
    on_change: Callback<f64>,
) -> NodeId {
    let (text, set_text) = create_signal(format_number(value.peek()));
    let (editing, set_editing) = create_signal(false);
    let (refocus, set_refocus) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    create_effect(clone!(value text set_text -> move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse(&held) != Some(next) {
            set_text.set(format_number(next));
        }
    }));
    let off = create_memo(clone!(disabled -> move || disabled.get()));
    let idle = create_memo(clone!(editing -> move || !editing.get()));
    let cursor = create_memo(clone!(off -> move || {
        match drag != NumberDrag::Off && !off.get() {
            true => CursorIcon::ResizeHorizontal,
            false => CursorIcon::Default,
        }
    }));
    let accessibility = create_memo(clone!(label text off -> move || {
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
    let original = Rc::new(Cell::new(value.peek()));
    let open = clone!(off set_editing original value -> move || {
        if !off.get_untracked() {
            original.set(value.peek());
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

    let restore = on_change.clone();
    let edited = clone!(set_text -> move |typed: String| {
        set_text.set(typed.clone());
        if let Some(parsed) = parse(&typed) {
            on_change.call(parsed.clamp(min, max));
        }
    });
    let close = clone!(value text set_text set_editing -> move || {
        let shown = value.peek();
        if parse(&text.get_untracked()) != Some(shown) {
            set_text.set(format_number(shown));
        }
        set_editing.set(false);
    });
    let submitted = clone!(close set_refocus -> move |_: String| {
        set_refocus.set(true);
        close();
    });
    let focus_changed = clone!(close -> move |focused: bool| {
        if !focused {
            close();
        }
    });
    let escaped = clone!(value set_text set_editing set_refocus -> move |press: KeyPress| {
        if press.key != Key::Escape {
            return false;
        }
        let before = original.get();
        set_text.set(format_number(before));
        if value.peek() != before {
            restore.call(before);
        }
        set_refocus.set(true);
        set_editing.set(false);
        true
    });
    let button_focus = move |has_focus: bool| {
        set_focused.set(has_focus);
        if !has_focus {
            set_refocus.set(false);
        }
    };
    let tab_stop = off.clone().into_prop().map(|off: bool| !off);

    let face_text = text.clone();
    let face_placeholder = placeholder.clone();
    let face_off = off.clone();
    let field = NodeRef::new();
    let face = NodeRef::new();
    set_component_state(Field {
        field: field.clone(),
        face: face.clone(),
        editing: editing.clone(),
    });
    view! {
        <List spacing=0.0>
            <Show condition={idle}>
                <Focusable
                    tab_stop
                    focused={refocus}
                    on_focus_change={button_focus}
                    on_activate={open.clone()}
                >
                    <ClickCatcher
                        cursor={cursor}
                        capture_presses=true
                        on_press={pressed}
                        on_drag={dragged}
                        on_click={clicked}
                        on_hover_change={move |is_hovered: bool| set_hovered.set(is_hovered)}
                    >
                        <NumberFace
                            shown_text={face.clone()}
                            text={face_text}
                            placeholder={face_placeholder}
                            hovered={hovered}
                            focused={focused}
                            disabled={face_off}
                        />
                    </ClickCatcher>
                </Focusable>
            </Show>
            <Show condition={editing.clone()}>
                <TextInput
                    @node_ref=&field
                    value={text}
                    label={label}
                    placeholder={placeholder}
                    disabled={disabled}
                    focused={editing}
                    select_on_focus=true
                    on_change={edited}
                    on_submit={submitted}
                    on_focus_change={focus_changed}
                    on_key_override={escaped}
                />
            </Show>
        </List>
    }
}

#[component]
fn NumberFace(
    shown_text: NodeRef,
    text: ReadSignal<String>,
    placeholder: Prop<String>,
    hovered: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    disabled: Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let placeholder = create_memo(move || placeholder.get());
    let shown = create_memo(clone!(text placeholder -> move || {
        let text = text.get();
        match text.is_empty() {
            true => placeholder.get(),
            false => text,
        }
    }));
    let color = create_memo(clone!(theme text disabled -> move || {
        match disabled.get() || text.get().is_empty() {
            true => theme.text_muted.get(),
            false => theme.text.get(),
        }
    }));
    let fill = create_memo(clone!(theme disabled hovered -> move || {
        match (disabled.get(), hovered.get()) {
            (true, _) => theme.surface.get(),
            (false, true) => theme.hover.get(),
            (false, false) => theme.surface_raised.get(),
        }
    }));
    let border = create_memo(clone!(theme disabled hovered -> move || {
        match (disabled.get(), hovered.get()) {
            (false, true) => theme.text_muted.get(),
            _ => theme.border.get(),
        }
    }));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame
                height=HEIGHT
                color={fill}
                outline={border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                padding_horizontal=PADDING_HORIZONTAL
            >
                <Text
                    @node_ref=&shown_text
                    string={shown}
                    font_size=FONT_BODY
                    color={color}
                    align=TextAlign::Start
                    clip=true
                />
            </Frame>
        </Frame>
    }
}

pub fn number_input_field(document: &Document, input: NodeId) -> Option<NodeId> {
    let state = document.component_state::<Field>(input);
    state.editing.get_untracked().then(|| state.field.get())
}

pub fn number_input_text(document: &Document, input: NodeId) -> Option<NodeId> {
    let state = document.component_state::<Field>(input);
    (!state.editing.get_untracked()).then(|| state.face.get())
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
