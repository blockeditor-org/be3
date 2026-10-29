use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::Arc;

use accesskit::{Node, Role};

use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

use beui_view::types::Color;
use beui_view::types::ItemSize;
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
    pub clear: ClickCallback,
    pub copy: ClickCallback,
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
}

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
    api.fill(TextInputApi(Input {
        state,
        area,
        menu: menu_node,
        focused: is_focused.read_only(),
        browser,
    }));

    create_effect(move || {
        let value = value.get();
        if text_of(state) != value {
            state.execute(EditorCommand::ReplaceWholeFile(value.as_bytes()));
        }
    });
    let edited = state.content();
    let empty = create_memo(move || {
        edited.get();
        text_of(state).is_empty()
    });
    let clear = ClickCallback::new(move || {
        state.execute(EditorCommand::ReplaceWholeFile(b""));
    });
    let copy = ClickCallback::new(move || copy_text(text_of(state)));
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
        (true, None) => ClickCallback::empty(),
        _ => ClickCallback::new(move || {
            on_submit.call(text_of(state));
            if let Some(form) = form
                && !browser
            {
                form.call();
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
        let clear = ClickCallback::new(move || value.set(String::new()));
        let copy = ClickCallback::new(move || copy_text(value.get_untracked()));
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
                child_size={match padded {
                    true => ItemSize::Intrinsic,
                    false => ItemSize::Percent(100.0),
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
    match multiline {
        true => surround(area_node, false),
        false => area_node,
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
        view! {
            <html::Input
                r#type={Prop::dynamic(move || Some(kind.get().html_type().to_owned()))}
                value={Prop::dynamic(move || Some(value.get()))}
                name={Prop::dynamic(move || present(name.get()))}
                autocomplete={Prop::dynamic(move || present(autocomplete.get()))}
                placeholder={Prop::dynamic(move || present(placeholder.get()))}
                disabled={disabled}
                @attr={("spellcheck", "false".to_owned())}
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
        }
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
    let state = TextAreaState::new(buffer as Arc<dyn text_editor_core::Document>);
    state.execute(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = state.with_core(|core| core.position(value.len()));
    state.execute(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    state
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

    pub fn caret(&self) -> usize {
        self.0.state.caret_indices().first().copied().unwrap_or(0)
    }

    pub fn state(&self) -> TextAreaState {
        self.0.state
    }

    pub fn selection(&self) -> Vec<Range<usize>> {
        self.0
            .state
            .selection_ranges()
            .into_iter()
            .filter(|range| range.start < range.end)
            .collect()
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
