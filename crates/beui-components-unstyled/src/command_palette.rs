use stable_hash::HashMap;
use std::rc::Rc;

use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Image;
use beui_view::types::icons::Glyph;

use crate::modal::{Modal, ModalHandle, ModalMotion};
use crate::scroll::ScrollbarStyle;
use crate::search_list::{SearchList, SearchListApi, SearchRowHandle, fuzzy_score, rank};
use crate::text_input::{TextInputHandle, TextInputStyle};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaletteItem {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub terms: Vec<String>,
    pub glyph: Option<Glyph>,
    pub image: Option<Image>,
    pub shortcut: String,
    pub checked: Option<bool>,
    pub disabled: bool,
}

impl PaletteItem {
    pub fn substring_score(&self, query: &str) -> Option<u32> {
        crate::search_list::substring_score(&self.title, query)
    }

    pub fn fuzzy_score(&self, query: &str) -> Option<u32> {
        fuzzy_score(&self.title, &self.terms, &self.detail, query)
    }

    fn from_action(action: &Action) -> Self {
        Self {
            key: action.id().to_owned(),
            title: action.label().get(),
            glyph: action.glyph(),
            shortcut: action.shortcut_label().unwrap_or_default(),
            checked: action.checked().map(|checked| checked.get()),
            disabled: !action.enabled().get(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Default)]
pub enum PaletteItems {
    #[default]
    Actions,
    List(Prop<Vec<PaletteItem>>),
}

impl PaletteItems {
    pub fn list(items: impl IntoProp<Vec<PaletteItem>>) -> Self {
        Self::List(items.into_prop())
    }
}

pub type PaletteScore = fn(&PaletteItem, &str) -> Option<u32>;

pub struct CommandRowHandle {
    pub key: String,
    pub label: Memo<String>,
    pub detail: Memo<String>,
    pub glyph: Memo<Option<Glyph>>,
    pub image: Memo<Option<Image>>,
    pub shortcut: Memo<String>,
    pub checked: Memo<Option<bool>>,
    pub disabled: Memo<bool>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

#[derive(Clone, Copy)]
struct State {
    list: ComponentRef<SearchListApi<String>>,
    actions: Signal<Rc<Vec<Action>>>,
    listed: Memo<Vec<PaletteItem>>,
    by_key: Memo<Rc<HashMap<String, usize>>>,
    query: Signal<String>,
    highlighted: Signal<Option<String>>,
    searching: Signal<bool>,
    open: Binding<bool>,
    lists_actions: bool,
    on_choose: Callback<String>,
}

impl State {
    fn item(&self, key: &str) -> Option<PaletteItem> {
        let at = self.by_key.with(|keys| keys.get(key).copied())?;
        self.listed.with(|items| items.get(at).cloned())
    }
}

#[component]
pub fn CommandPalette(
    #[prop(default = ComponentRef::new())] api: ComponentRef<CommandPaletteApi>,
    open: Binding<bool>,
    #[prop(default = PaletteItems::Actions)] items: PaletteItems,
    #[prop(default = PaletteItem::substring_score as PaletteScore)] score: PaletteScore,
    #[prop(default = create_signal(String::new()))] query: Signal<String>,
    on_choose: Callback<String>,
    #[prop(default = "Command palette".to_owned())] label: Prop<String>,
    #[prop(default = None)] shown_in: Prop<Option<beui_view::types::WindowId>>,
    list_height: Prop<f32>,
    spacing: f32,
    scrim: Prop<Color>,
    search_placeholder: Prop<String>,
    search_content: RenderFn<TextInputHandle>,
    search_style: TextInputStyle,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    row: RenderFn<CommandRowHandle>,
    #[prop(children)] panel: Option<RenderFn<Child>>,
) -> NodeId {
    let panel = panel.unwrap_or_else(|| RenderFn::new(|content| content));
    let lists_actions = matches!(items, PaletteItems::Actions);
    let actions = create_signal(Rc::new(Vec::<Action>::new()));
    let listed = create_memo(move || match &items {
        PaletteItems::List(items) => items.get(),
        PaletteItems::Actions => {
            actions.with(|actions| actions.iter().map(PaletteItem::from_action).collect())
        }
    });
    let by_key = create_memo(move || {
        listed.with(|items| {
            Rc::new(
                items
                    .iter()
                    .enumerate()
                    .map(|(at, item)| (item.key.clone(), at))
                    .collect::<HashMap<_, _>>(),
            )
        })
    });
    let state = State {
        list: ComponentRef::new(),
        actions,
        listed,
        by_key,
        query,
        highlighted: create_signal(None),
        searching: create_signal(false),
        open,
        lists_actions,
        on_choose,
    };
    api.fill(CommandPaletteApi(state));

    create_effect(move || {
        if open.get() {
            untrack(|| start(state));
        }
    });

    let search_placeholder = create_memo(move || search_placeholder.get());
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::Dialog);
        node.set_label(label.get());
        node
    });
    let ranked = move |query: String| {
        listed.with(|items| {
            rank(items, &query, score)
                .into_iter()
                .map(|at| items[at].key.clone())
                .collect::<Vec<_>>()
        })
    };
    let enabled = move |key: String| state.item(&key).is_some_and(|item| !item.disabled);
    let row_accessibility = move |key: String| {
        let mut node = Node::new(Role::ListBoxOption);
        if let Some(item) = state.item(&key) {
            node.set_label(item.title);
            if let Some(on) = item.checked {
                node.set_toggled(Toggled::from(on));
            }
            if item.disabled {
                node.set_disabled();
            }
        }
        node
    };
    let palette_row = move |handle: SearchRowHandle<String>| {
        let key = handle.key;
        let item = create_memo({
            let key = key.clone();
            move || state.item(&key)
        });
        let text = move |read: fn(&PaletteItem) -> String| {
            create_memo(move || item.with(|item| item.as_ref().map(read).unwrap_or_default()))
        };
        let enabled = handle.enabled;
        row.call(CommandRowHandle {
            key,
            label: text(|item| item.title.clone()),
            detail: text(|item| item.detail.clone()),
            glyph: create_memo(move || item.with(|item| item.as_ref()?.glyph)),
            image: create_memo(move || item.with(|item| item.as_ref()?.image.clone())),
            shortcut: text(|item| item.shortcut.clone()),
            checked: create_memo(move || item.with(|item| item.as_ref()?.checked)),
            disabled: create_memo(move || !enabled.get()),
            highlighted: handle.highlighted,
            hovered: handle.hovered,
        })
    };
    view! {
        <Modal
            open={create_memo(move || open.get())}
            scrim
            motion=ModalMotion::Still
            shown_in
            on_dismiss={move || dismiss(state)}
        >|_: ModalHandle| {
            panel.call(view! {
                <SearchList
                    api={state.list}
                    query
                    highlighted={state.highlighted}
                    rank={ranked}
                    enabled
                    on_choose={move |key: String| choose(state, key)}
                    focused={state.searching}
                    placeholder={search_placeholder}
                    style={search_style}
                    search_accessibility={accessibility}
                    field={search_content}
                    list_height
                    spacing
                    scrollbar
                    accessibility={row_accessibility}
                    row={palette_row}
                />
            })
        }</Modal>
    }
}

fn start(state: State) {
    if state.lists_actions {
        let mut actions = active_actions_from(&focus_ancestry());
        actions.sort_by_key(|action| !action.is_enabled());
        state.actions.set(Rc::new(actions));
    }
    state.query.set(String::new());
    state.highlighted.set(None);
    state.searching.set(true);
}

fn close(state: State) {
    state.searching.set(false);
    state.open.set(false);
}

fn dismiss(state: State) {
    if state.searching.get_untracked() {
        close(state);
    }
}

fn choose(state: State, key: String) {
    let action = state.lists_actions.then(|| {
        state
            .actions
            .with_untracked(|actions| actions.iter().find(|action| action.id() == key).cloned())
    });
    close(state);
    if let Some(action) = action.flatten() {
        action.run();
    }
    state.on_choose.call(key);
}

#[derive(Clone, Copy)]
pub struct CommandPaletteApi(State);

impl CommandPaletteApi {
    pub fn search(&self) -> NodeId {
        self.0.list.get().search()
    }

    pub fn shown(&self) -> Vec<String> {
        self.0
            .list
            .try_get()
            .map(|list| list.shown())
            .unwrap_or_default()
    }

    pub fn highlighted(&self) -> Option<String> {
        self.0.highlighted.get_untracked()
    }

    pub fn row(&self, key: &str) -> Option<NodeId> {
        self.0.list.try_get()?.row(&key.to_owned())
    }
}
