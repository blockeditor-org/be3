use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::Arc;

use accesskit::{Node, Role};

use beui_text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

use beui_view::types::Color;
use beui_view::types::{CursorIcon, FontId, Pos2, Rect};

use crate::ContextMenuApi;
use crate::MenuListApi;
use crate::Prose;
use crate::SyntaxColors;
use crate::TextArea;
use crate::TextAreaApi;
use crate::TextAreaColors;
use crate::TextAreaGeometry;
use crate::TextAreaLook;
use crate::TextAreaState;
use crate::form::Posted;
use crate::menu_popup::MenuStyle;
use crate::text_menu::TextContextMenu;
use beui_macros::{component, view};
use beui_view::reactive::NodeId;

use beui_view::escape::beui_html::{Event, attr, elements as html};
use beui_view::escape::{Escape, html_escape};
use beui_view::primitives::*;
use beui_view::reactive::*;
use beui_view::types::Sides;
use beui_view::{CORE, Core};

pub struct TextInputHandle {
    pub field: Child,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub empty: Memo<bool>,
    pub clear: Callback<()>,
    pub copy: Callback<()>,
}

#[derive(Clone, Copy)]
pub struct TextInputStyle {
    pub font_size: Prop<f32>,
    pub color: Prop<Color>,
    pub placeholder_color: Prop<Color>,
    pub selection_color: Prop<Color>,
    pub caret_color: Prop<Color>,
    pub padding_horizontal: Prop<f32>,
    pub padding_vertical: Prop<f32>,
    pub prose: Prop<Option<Prose>>,
    pub geometry: Prop<TextAreaGeometry>,
    pub menu: Option<MenuStyle>,
}

/// The kind of a single-line field: the `<input type>` on the HTML core.
/// Natively only `Password` changes anything (it masks). A new kind of field is a
/// variant here rather than a component of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputKind {
    #[default]
    Text,
    Password,
    Email,
    Search,
    Url,
    Tel,
}

impl InputKind {
    pub fn html_type(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Password => "password",
            Self::Email => "email",
            Self::Search => "search",
            Self::Url => "url",
            Self::Tel => "tel",
        }
    }

    pub fn masked(self) -> bool {
        self == Self::Password
    }
}

#[derive(Clone, Copy)]
struct Input {
    state: TextAreaState,
    area: NodeRef,
    menu: NodeRef,
    focused: ReadSignal<bool>,
    browser: bool,
    /// The `<input>` on the HTML core, whose selection the browser owns.
    element: Stored<RefCell<Option<beui_view::escape::beui_html::Node>>>,
}

/// A single-line field (or, with `multiline`, a text area) bound to `value`.
///
/// On the HTML core, when not `multiline`, without `prose` and without
/// `select_on_focus`, it is a real `<input>` through the platform escape, so the
/// browser gives it IME, autofill and password managers; `autocomplete` only matters
/// there. `name` posts the value to a `Form` on HTML either way (a beui-drawn field
/// through a hidden input). Otherwise it is a `TextArea` in single-line
/// mode on both cores, so a fix to text editing lands in both.
#[component]
pub fn TextInput(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TextInputApi>,
    value: Binding<String>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] keyboard_on_focus: bool,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = InputKind::Text)] kind: Prop<InputKind>,
    #[prop(default = String::new())] name: Prop<String>,
    #[prop(default = String::new())] autocomplete: Prop<String>,
    #[prop(default = false)] select_on_focus: Prop<bool>,
    #[prop(default = false)] multiline: bool,
    #[prop(children)] content: Option<Render<TextInputHandle>>,
    placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    style: TextInputStyle,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let TextInputStyle {
        font_size,
        color,
        placeholder_color,
        selection_color,
        caret_color,
        padding_horizontal,
        padding_vertical,
        prose,
        geometry,
        menu,
    } = style;
    let initial = value.get_untracked();
    let state = plain_text(&initial);
    let hovered = create_signal(false);
    let is_focused = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let masked = create_memo(move || kind.get().masked());
    let (area, menu_node) = (NodeRef::new(), NodeRef::new());
    let browser = CORE == Core::Html
        && !multiline
        && untrack(|| prose.get().is_none() && !select_on_focus.get());
    let element = store(RefCell::new(None));
    api.fill(TextInputApi(Input {
        state,
        area,
        menu: menu_node,
        focused: is_focused.read_only(),
        browser,
        element,
    }));

    create_effect(move || {
        let value = value.get();
        if text_of(state) != value {
            let whole = untrack(|| select_on_focus.get() && is_focused.get()) && selects_all(state);
            state.execute(EditorCommand::ReplaceWholeFile(value.as_bytes()));
            if whole {
                state.execute(EditorCommand::SelectAll);
            }
        }
    });
    let edited = state.content();
    let empty = create_memo(move || {
        edited.get();
        text_of(state).is_empty()
    });
    let clear = Callback::new(move |()| {
        state.execute(EditorCommand::ReplaceWholeFile(b""));
    });
    let copy = Callback::new(move |()| beui_view::reactive::copy(Clip::text(text_of(state))));
    let content_changed = state.content();
    create_effect(move || {
        content_changed.get();
        value.set(text_of(state));
    });

    let colors = create_memo(move || TextAreaColors {
        selection: selection_color.get(),
        caret: caret_color.get(),
        placeholder: placeholder_color.get(),
        syntax: SyntaxColors::uniform(color.get()),
        gutter_border: Color::TRANSPARENT,
        gutter_text: Color::TRANSPARENT,
        gutter_arrow: Color::TRANSPARENT,
        code_background: Color::TRANSPARENT,
        widget: Color::TRANSPARENT,
        broken_widget: Color::TRANSPARENT,
        remote_selection_alpha: 0,
    });
    let role = match multiline {
        true => Role::MultilineTextInput,
        false => Role::TextInput,
    };
    let accessibility = crate::labelled_node(role, accessibility, label);
    let form = crate::form::form_submit();
    let submit = match (on_submit.is_empty(), form) {
        (true, None) => Callback::empty(),
        _ => Callback::new(move |()| {
            on_submit.call(text_of(state));
            if let Some(form) = form
                && !browser
            {
                form.call(());
            }
        }),
    };

    if browser {
        let field = view! {
            <BrowserField
                @node_ref=&area
                value
                kind
                name
                autocomplete
                placeholder
                disabled={disabled}
                focused
                keyboard_on_focus
                element
                style
                accessibility
                on_submit={move |_: String| on_submit.call(value.get_untracked())}
                on_focus_change={move |focused: bool| {
                    is_focused.set(focused);
                    on_focus_change.call(focused);
                }}
                on_hover_change={move |is_hovered: bool| {
                    hovered.set(is_hovered);
                    on_hover_change.call(is_hovered);
                }}
            />
        };
        let clear = Callback::new(move |()| value.set(String::new()));
        let copy = Callback::new(move |()| {
            beui_view::reactive::copy(Clip::text(value.get_untracked()))
        });
        return match content {
            Some(build) => build.call(TextInputHandle {
                field,
                hovered: hovered.read_only(),
                focused: is_focused.read_only(),
                disabled,
                empty,
                clear,
                copy,
            }),
            None => field,
        };
    }

    let opener = store(Cell::new(None::<Callback<Option<Rect>>>));
    let open_menu = move |at: Option<Rect>| {
        if let Some(open) = opener.with(Cell::get) {
            open.call(at);
        }
    };
    let surround = move |field: Child, padded: bool| {
        let field = match padded {
            true => view! {
                <Frame
                    padding={create_memo(move || Sides::symmetric(padding_horizontal.get(), padding_vertical.get()))}
                >
                    {field}
                </Frame>
            },
            false => field,
        };
        view! {
            <TextContextMenu
                @node_ref=&menu_node
                state={state}
                menu
                masked={masked}
                disabled={disabled}
                child_grow={match padded {
                    true => 0.0,
                    false => 1.0,
                }}
            >|open: Callback<Option<Rect>>| {
                opener.with(|opener| opener.set(Some(open)));
                match content {
                    Some(build) => build.call(TextInputHandle {
                        field,
                        hovered: hovered.read_only(),
                        focused: is_focused.read_only(),
                        disabled,
                        empty,
                        clear,
                        copy,
                    }),
                    None => field,
                }
            }</TextContextMenu>
        }
    };
    let surround = store(RefCell::new(Some(surround)));
    let surround = move |field: Child, padded: bool| {
        let wrap = surround
            .with(|surround| surround.borrow_mut().take())
            .expect("a text input wraps its field once");
        wrap(field, padded)
    };
    let padding = create_memo(move || match multiline {
        true => Sides::symmetric(padding_horizontal.get(), padding_vertical.get()),
        false => Sides::ZERO,
    });
    let area_node = view! {
        <TextArea
            @node_ref=&area
            state={state}
            single_line={!multiline}
            line_numbers=false
            colors
            look={TextAreaLook::plain(geometry)}
            placeholder
            password={masked}
            focused
            keyboard_on_focus
            disabled={disabled}
            font_size
            padding
            prose
            accessibility
            on_toolbar={open_menu}
            on_submit={submit}
            on_focus_change={move |focused: bool| {
                if focused && select_on_focus.get_untracked() {
                    state.execute(EditorCommand::SelectAll);
                }
                is_focused.set(focused);
                on_focus_change.call(focused);
            }}
            on_hover_change={move |is_hovered: bool| {
                hovered.set(is_hovered);
                on_hover_change.call(is_hovered);
            }}
            frame={move |field: Child| surround(field, true)}
        />
    };
    let field = match multiline {
        true => surround(area_node, false),
        false => area_node,
    };
    let posted = create_memo(move || Some(value.get()));
    view! {
        <Posted name value={posted} disabled content={field} />
    }
}

#[component]
fn BrowserField(
    value: Binding<String>,
    kind: Prop<InputKind>,
    name: Prop<String>,
    autocomplete: Prop<String>,
    placeholder: Prop<String>,
    disabled: Memo<bool>,
    focused: Prop<bool>,
    keyboard_on_focus: bool,
    element: Stored<RefCell<Option<beui_view::escape::beui_html::Node>>>,
    style: TextInputStyle,
    accessibility: Prop<Node>,
    on_submit: Callback<String>,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
) -> NodeId {
    let TextInputStyle {
        font_size,
        color,
        placeholder_color,
        selection_color,
        caret_color,
        padding_horizontal,
        padding_vertical,
        ..
    } = style;
    let shown = placeholder.clone();
    let named = accessibility.clone();
    let label = Prop::dynamic(move || named.get().label().map(str::to_owned));
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        let text = value.get();
        node.set_value(match kind.get().masked() {
            true => crate::text_area::rows::mask(&text),
            false => text,
        });
        let placeholder = shown.get();
        if !placeholder.is_empty() {
            node.set_placeholder(placeholder);
        }
        match disabled.get() {
            true => node.set_disabled(),
            false => node.clear_disabled(),
        }
        node
    }));
    let present = |text: String| Some(text).filter(|text| !text.is_empty());
    let field = html_escape(move |escape| {
        escape.style("align-self", "stretch".to_owned());
        escape.style("width", "100%".to_owned());
        escape.style(
            "font",
            Prop::dynamic(move || Some(Escape::font(FontId::proportional(font_size.get())))),
        );
        escape.style(
            "color",
            Prop::dynamic(move || Some(Escape::color(color.get()))),
        );
        escape.style(
            "caret-color",
            Prop::dynamic(move || Some(Escape::color(caret_color.get()))),
        );
        escape.style(
            "--beui-placeholder",
            Prop::dynamic(move || Some(Escape::color(placeholder_color.get()))),
        );
        escape.style(
            "--beui-selection",
            Prop::dynamic(move || Some(Escape::color(selection_color.get()))),
        );
        escape.style(
            "padding",
            Prop::dynamic(move || {
                Some(format!(
                    "{}px {}px",
                    padding_vertical.get(),
                    padding_horizontal.get()
                ))
            }),
        );
        let take = move |event: &Event| {
            if let Some(text) = event.target_value() {
                value.set(text);
            }
        };
        // Without `keyboard_on_focus` the browser's keyboard opens on a press, not when
        // focus arrives some other way: the policy is manual until a press lands.
        let pressed = create_signal(false);
        let input = view! {
            <html::Input
                r#type={Prop::dynamic(move || Some(kind.get().html_type().to_owned()))}
                value={Prop::dynamic(move || Some(value.get()))}
                name={Prop::dynamic(move || present(name.get()))}
                autocomplete={Prop::dynamic(move || present(autocomplete.get()))}
                placeholder={Prop::dynamic(move || present(placeholder.get()))}
                disabled={disabled}
                @attr={("spellcheck", "false".to_owned())}
                @attr={(
                    "virtualkeyboardpolicy",
                    Prop::dynamic(move || (!keyboard_on_focus && !pressed.get()).then(|| "manual".to_owned())),
                )}
                on_pointerdown={move |_: Event| pressed.set(true)}
                on_blur={move |_: Event| pressed.set(false)}
                @attr={("aria-label", label)}
                on_input={move |event: Event| take(&event)}
                on_change={move |event: Event| take(&event)}
                on_keydown={move |event: Event| {
                    if event.composing() {
                        return;
                    }
                    let key = event.key().unwrap_or_default();
                    if key == "Enter" {
                        take(&event);
                        if !on_submit.is_empty() {
                            on_submit.call(value.get_untracked());
                            event.stop_propagation();
                        }
                        return;
                    }
                    if edits(&key, event.holds("Control") || event.holds("Meta")) {
                        event.stop_propagation();
                    }
                }}
            />
        };
        element.with(|element| *element.borrow_mut() = Some(input.clone()));
        input
    });
    view! {
        <Interactive
            focusable=true
            focused
            on_focus_change={on_focus_change}
            on_hover_change={on_hover_change}
            cursor={create_memo(move || (!disabled.get()).then_some(CursorIcon::Text))}
        >
            {field}
        </Interactive>
    }
}

fn edits(key: &str, command: bool) -> bool {
    match command {
        true => matches!(
            key.to_ascii_lowercase().as_str(),
            "a" | "c" | "v" | "x" | "z" | "y" | "backspace" | "delete" | "arrowleft" | "arrowright"
        ),
        false => {
            key.chars().count() == 1
                || matches!(
                    key,
                    "Backspace" | "Delete" | "ArrowLeft" | "ArrowRight" | "Home" | "End"
                )
        }
    }
}

fn plain_text(value: &str) -> TextAreaState {
    let buffer = Arc::new(TextBuffer::new(value.as_bytes()));
    let state = TextAreaState::new(buffer as Arc<dyn beui_text_editor_core::Document>);
    state.execute(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = state.with_core(|core| core.position(value.len()));
    state.execute(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    state
}

fn selects_all(state: TextAreaState) -> bool {
    let length = text_of(state).len();
    length == 0 || state.selection_ranges() == [0..length]
}

fn text_of(state: TextAreaState) -> String {
    state.with_core(|core| {
        let Some(read) = core.document().read() else {
            return String::new();
        };
        String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
    })
}

#[derive(Clone, Copy)]
pub struct TextInputApi(Input);

impl TextInputApi {
    pub fn text(&self) -> NodeId {
        match self.0.browser {
            true => self.0.area.get(),
            false => self.0.state.canvas().get(),
        }
    }

    pub fn shown(&self, tree: &impl Inspect) -> String {
        if self.0.browser {
            return text_of(self.0.state);
        }
        tree.api::<TextAreaApi>(self.0.area.get()).shown()
    }

    pub fn index_at(&self, tree: &impl Inspect, pos: Pos2) -> usize {
        if self.0.browser {
            return 0;
        }
        tree.api::<TextAreaApi>(self.0.area.get())
            .index_at(tree, pos)
    }

    pub fn value(&self) -> String {
        text_of(self.0.state)
    }

    pub fn focused(&self) -> ReadSignal<bool> {
        self.0.focused
    }

    /// The caret's byte index: the selection's focus, which comes first in a selection made
    /// backwards. On the HTML core the browser owns it, and a kind of `<input>` with no
    /// selection there (`Email`) reports its end.
    pub fn caret(&self) -> usize {
        if self.0.browser {
            return match self.browser_selection() {
                Some(selection) => selection.caret(),
                None => self.value().len(),
            };
        }
        self.0.state.caret_indices().first().copied().unwrap_or(0)
    }

    pub fn state(&self) -> TextAreaState {
        self.0.state
    }

    /// The selected byte ranges, none when only a caret shows. On the HTML core these are
    /// the `<input>`'s own, and none for a kind with no selection there (`Email`).
    pub fn selection(&self) -> Vec<Range<usize>> {
        if self.0.browser {
            return self
                .browser_selection()
                .map(|selection| selection.range)
                .filter(|range| range.start < range.end)
                .into_iter()
                .collect();
        }
        self.0
            .state
            .selection_ranges()
            .into_iter()
            .filter(|range| range.start < range.end)
            .collect()
    }

    /// Selects the byte range `range` of the value (a caret when it is empty), clamped to
    /// the value and to character boundaries. On the HTML core it does nothing for a kind
    /// with no selection there (`Email`).
    pub fn set_selection(&self, range: Range<usize>) {
        if self.0.browser {
            self.0.element.with(|element| {
                let element = element.borrow();
                let Some(input) = element.as_ref() else {
                    return;
                };
                let Some(value) = input.string("value") else {
                    return;
                };
                let (start, end) = clamp_range(&value, range);
                let units = |index: usize| value[..index].encode_utf16().count() as f64;
                if input.number("selectionStart").is_some() {
                    input.set_number("selectionEnd", units(end));
                    input.set_number("selectionStart", units(start));
                }
            });
            return;
        }
        let (start, end) = clamp_range(&self.value(), range);
        let (anchor, focus) = self
            .0
            .state
            .with_core(|core| (core.position(start), core.position(end)));
        self.0
            .state
            .execute(EditorCommand::SetSelection { anchor, focus });
    }

    /// The `<input>`'s selection on the HTML core, as byte indices into the value the
    /// browser holds (which it may have trimmed), or `None` for a kind without one.
    fn browser_selection(&self) -> Option<BrowserSelection> {
        self.0.element.with(|element| {
            let element = element.borrow();
            let input = element.as_ref()?;
            let value = input.string("value")?;
            let start = utf16_to_byte(&value, input.number("selectionStart")?);
            let end = utf16_to_byte(&value, input.number("selectionEnd")?);
            Some(BrowserSelection {
                range: start..end,
                backward: input.string("selectionDirection").as_deref() == Some("backward"),
            })
        })
    }

    pub fn menu_row(&self, tree: &impl Inspect, index: usize) -> Option<NodeId> {
        let menu = self.0.menu.try_get()?;
        if !tree.api::<ContextMenuApi>(menu).open(tree) {
            return None;
        }
        let list = tree.api::<ContextMenuApi>(menu).menu();
        (index < tree.api::<MenuListApi>(list).row_count())
            .then(|| tree.api::<MenuListApi>(list).row_button(index))
    }

    pub fn toolbar(&self, tree: &impl Inspect) -> Option<NodeId> {
        let menu = self.0.menu.try_get()?;
        tree.api::<ContextMenuApi>(menu).toolbar(tree)
    }

    pub fn handles(&self, tree: &impl Inspect) -> Vec<Pos2> {
        if self.0.browser {
            return Vec::new();
        }
        tree.api::<crate::TextAreaApi>(self.0.area.get())
            .handles(tree)
    }
}

struct BrowserSelection {
    range: Range<usize>,
    backward: bool,
}

impl BrowserSelection {
    fn caret(&self) -> usize {
        match self.backward {
            true => self.range.start,
            false => self.range.end,
        }
    }
}

/// `range` ordered, clamped to `value` and moved back onto character boundaries.
fn clamp_range(value: &str, range: Range<usize>) -> (usize, usize) {
    let boundary = |index: usize| {
        let mut index = index.min(value.len());
        while !value.is_char_boundary(index) {
            index -= 1;
        }
        index
    };
    (boundary(range.start), boundary(range.end.max(range.start)))
}

/// The byte index in `value` of a UTF-16 offset, as the DOM counts; an offset inside a
/// surrogate pair rounds up to the character's end, and one past the end clamps to it.
fn utf16_to_byte(value: &str, units: f64) -> usize {
    let mut counted = 0.0;
    value
        .char_indices()
        .find(|(_, char)| {
            let before = counted;
            counted += char.len_utf16() as f64;
            before >= units
        })
        .map_or(value.len(), |(index, _)| index)
}
