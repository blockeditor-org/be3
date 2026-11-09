//! The one command palette be-host draws (`open-block.host-palette`): the actions of the
//! instance that has the focus and of every instance around it, innermost first, each under its
//! block's name, then the host's own; or, for quick open, blocks to open.

use std::collections::BTreeSet;
use std::rc::Rc;

use be_sdk_api::{EditorInstanceId, PaletteMode, ViewId};
use beui::icons::ICON_FILE;
use beui::native::try_with_document;
use beui::reactive::*;
use beui::styled::*;
use beui::{Key, KeyEvent, KeyInput, NodeId};
use uuid::Uuid;

const PALETTE: Chord = Chord::ctrl(Key::P).shift();

#[derive(Clone, PartialEq)]
struct Row {
    item: PaletteItem,
    run: Run,
}

/// What choosing a row does.
#[derive(Clone, PartialEq)]
enum Run {
    /// Runs an instance's action (`app.run-action`) after giving its view the focus back.
    Action {
        catcher: NodeId,
        app: String,
        instance: EditorInstanceId,
        view: ViewId,
        id: String,
    },
    /// Runs one of the host's own actions.
    Host(Action),
    /// Opens a block, as the instance that asked for quick open would.
    Open {
        block: Uuid,
        block_type: Uuid,
        from: Option<Uuid>,
    },
}

/// A block quick open offers.
#[derive(Clone, PartialEq)]
pub(crate) struct OpenableBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) name: String,
    /// Where it is: the name of the block holding it.
    pub(crate) place: String,
}

/// Where quick open's rows come from, given what was typed. The host's own lists the blocks it
/// has loaded, those open in an editor first; a search (plans/search.md) answers with its hits.
pub(crate) type BlockSource = Rc<dyn Fn(&str) -> Vec<OpenableBlock>>;

#[derive(Clone, Copy, PartialEq)]
enum Shown {
    Actions,
    Blocks { from: Option<Uuid> },
}

#[component]
pub(crate) fn HostPalette(
    #[prop(default = Rc::new(loaded_blocks) as BlockSource)] blocks: BlockSource,
) -> NodeId {
    let open = create_signal(false);
    let shown = create_signal(Shown::Actions);
    let query = create_signal(String::new());
    let actions = create_signal(Rc::new(Vec::<Row>::new()));
    let rows = create_memo(move || match shown.get() {
        Shown::Actions => actions.get(),
        Shown::Blocks { from } => {
            let typed = query.get();
            Rc::new(block_rows(blocks(&typed), from))
        }
    });
    let items = create_memo(move || {
        rows.with(|rows| rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>())
    });
    let show = move |mode: PaletteMode, from: Option<NodeId>| {
        match mode {
            PaletteMode::Actions => {
                actions.set(Rc::new(actions_from(from)));
                shown.set(Shown::Actions);
            }
            PaletteMode::Blocks => {
                let from = from
                    .and_then(super::desktop::catcher)
                    .and_then(|catcher| catcher.block);
                shown.set(Shown::Blocks { from });
            }
        }
        open.set(true);
    };
    on_global_key(move |event: KeyEvent| {
        let KeyInput::Press(press) = event.input else {
            return false;
        };
        if event.held() || !PALETTE.matches(press) || leaves_to_program(event) {
            return false;
        }
        show(PaletteMode::Actions, focused_catcher());
        true
    });
    let any = super::any();
    create_effect(move || {
        any.get();
        untrack(|| {
            let asked = be_host_runtime::take_palette_asks().into_iter().last();
            if let Some(ask) = asked {
                show(ask.mode, catcher_of(&ask.app, ask.instance));
            }
        });
    });
    let placeholder = create_memo(move || match shown.get() {
        Shown::Actions => "Run a command".to_owned(),
        Shown::Blocks { .. } => "Go to a file".to_owned(),
    });
    let label = create_memo(move || match shown.get() {
        Shown::Actions => "Command palette".to_owned(),
        Shown::Blocks { .. } => "Quick open".to_owned(),
    });
    view! {
        <CommandPalette
            open
            items={PaletteItems::list(items)}
            score={PaletteItem::fuzzy_score as PaletteScore}
            query
            placeholder
            label
            id="host.palette"
            on_choose={move |key: String| choose(rows, &key)}
        />
    }
}

fn choose(rows: Memo<Rc<Vec<Row>>>, key: &str) {
    let chosen = rows.with_untracked(|rows| {
        rows.iter()
            .find(|row| row.item.key == key)
            .map(|row| row.run.clone())
    });
    match chosen {
        Some(Run::Action {
            catcher,
            app,
            instance,
            view,
            id,
        }) => {
            try_with_document(|document| document.focus_focusable(catcher));
            be_host_runtime::run_view_action(&app, instance, view, &id);
        }
        Some(Run::Host(action)) => {
            untrack(|| action.run());
        }
        Some(Run::Open {
            block,
            block_type,
            from,
        }) => super::act(
            from,
            be_host_runtime::EditorAction::OpenBlock(be_host_runtime::BlockOpen {
                id: block,
                block_type,
                via: None,
                editor: None,
            }),
        ),
        None => {}
    }
}

/// Whether the key goes to a focused program other than an app's view (a Wayland window),
/// which keeps its own Ctrl+Shift+P: the host's palette opens over be3's views only.
fn leaves_to_program(event: KeyEvent) -> bool {
    event.target.program
        && !try_with_document(|document| document.focused_node())
            .flatten()
            .is_some_and(|node| super::desktop::catcher(node).is_some())
}

fn block_rows(blocks: Vec<OpenableBlock>, from: Option<Uuid>) -> Vec<Row> {
    blocks
        .into_iter()
        .map(|block| Row {
            item: PaletteItem {
                key: block.id.to_string(),
                title: block.name,
                detail: block.place,
                glyph: Some(ICON_FILE),
                ..PaletteItem::default()
            },
            run: Run::Open {
                block: block.id,
                block_type: block.block_type,
                from,
            },
        })
        .collect()
}

/// The blocks the host has loaded that may be viewed and that an editor shows (named after
/// themselves, else their type), those open in an editor first. Quick open ranks them by what
/// was typed.
fn loaded_blocks(_typed: &str) -> Vec<OpenableBlock> {
    let editors = super::editors::editors();
    let open: Vec<Uuid> = editors.with(|open| open.keys().copied().collect());
    let registry = editors.registry();
    let mut blocks: Vec<OpenableBlock> = be_host_stack::with_graph(|graph| {
        graph
            .nodes()
            .filter(|node| node.access.can_view())
            .filter_map(|node| {
                let type_name = registry.display_name(node.content_type)?;
                let name = node.metadata.name().unwrap_or_else(|| type_name.to_owned());
                let place = match node.parent {
                    Some(be_block_graph::BlockParent::Block(parent)) => graph
                        .get(parent)
                        .and_then(|parent| parent.metadata.name())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                Some(OpenableBlock {
                    id: node.id,
                    block_type: node.content_type,
                    name,
                    place,
                })
            })
            .collect()
    })
    .unwrap_or_default();
    blocks.sort_by(|a, b| {
        (!open.contains(&a.id), &a.name, a.id).cmp(&(!open.contains(&b.id), &b.name, b.id))
    });
    blocks
}

/// The view that has the keyboard: the innermost app view on the focus path.
fn focused_catcher() -> Option<NodeId> {
    try_with_document(|document| document.focus_ancestry())
        .unwrap_or_default()
        .into_iter()
        .find(|node| super::desktop::catcher(*node).is_some())
}

/// The instance's view the palette lists from: the focused one, else its main view.
fn catcher_of(app: &str, instance: EditorInstanceId) -> Option<NodeId> {
    let views = super::desktop::catchers_of(app, instance);
    let focused = focused_catcher().filter(|node| views.contains(node));
    focused.or_else(|| {
        views
            .iter()
            .copied()
            .find(|node| super::desktop::catcher(*node).is_some_and(|held| held.region.is_main()))
            .or_else(|| views.first().copied())
    })
}

/// The live actions of the instance whose view `from` is, and of each instance placing it in
/// turn, out to the root, each instance's under its block's name: every view's of the
/// instance, the placing view's first. Ids repeat across instances (each has its Undo) but not
/// within one.
fn actions_from(from: Option<NodeId>) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut seen = Vec::new();
    let mut at = from;
    while let Some(node) = at.filter(|node| !seen.contains(node)) {
        seen.push(node);
        let Some(catcher) = super::desktop::catcher(node) else {
            break;
        };
        let name = catcher
            .block
            .and_then(be_host_stack::node)
            .and_then(|block| block.metadata.name())
            .unwrap_or_else(|| catcher.app_name.clone());
        let mut listed = BTreeSet::new();
        let mut actions: Vec<_> =
            be_host_runtime::instance_actions(&catcher.app, catcher.instance, catcher.region)
                .into_iter()
                .filter(|(_, action)| action.live && listed.insert(action.id.clone()))
                .collect();
        actions.sort_by_key(|(_, action)| !action.enabled);
        for (view, action) in actions {
            let key = format!(
                "{}/{}/{}/{}",
                catcher.app, catcher.instance.0, view.0, action.id
            );
            rows.push(Row {
                item: PaletteItem {
                    key,
                    title: action.label,
                    detail: name.clone(),
                    glyph: action.glyph.as_deref().and_then(beui::icons::named),
                    shortcut: action
                        .shortcut
                        .map(|chord| chord.label())
                        .unwrap_or_default(),
                    checked: action.checked,
                    disabled: !action.enabled,
                    ..PaletteItem::default()
                },
                run: Run::Action {
                    catcher: node,
                    app: catcher.app.clone(),
                    instance: catcher.instance,
                    view,
                    id: action.id,
                },
            });
        }
        at = catcher
            .placer
            .as_ref()
            .and_then(|(app, instance, view)| catcher_of_view(app, *instance, *view));
    }
    rows.extend(host_rows());
    rows
}

/// The host's own actions live where the focus is (switch account, lock, theme), after every
/// instance's.
fn host_rows() -> Vec<Row> {
    let path = try_with_document(|document| document.focus_ancestry()).unwrap_or_default();
    let mut actions: Vec<Action> = active_actions_from(&path)
        .into_iter()
        .filter(|action| action.id() != "host.palette" && action.is_alive())
        .collect();
    actions.sort_by_key(|action| !action.is_enabled());
    actions
        .into_iter()
        .map(|action| Row {
            item: PaletteItem {
                key: format!("host/{}", action.id()),
                title: action.label().get_untracked(),
                detail: "be-host".to_owned(),
                glyph: action.glyph(),
                shortcut: action.shortcut_label().unwrap_or_default(),
                checked: action.checked().map(|checked| checked.get_untracked()),
                disabled: !action.is_enabled(),
                ..PaletteItem::default()
            },
            run: Run::Host(action),
        })
        .collect()
}

/// The view of an instance that places children: the one named, else any the instance shows.
fn catcher_of_view(app: &str, instance: EditorInstanceId, view: ViewId) -> Option<NodeId> {
    let views = super::desktop::catchers_of(app, instance);
    views
        .iter()
        .copied()
        .find(|node| super::desktop::catcher(*node).is_some_and(|held| held.region == view))
        .or_else(|| views.first().copied())
}
