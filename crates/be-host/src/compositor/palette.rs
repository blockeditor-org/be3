//! The one command palette be-host draws (`open-block.host-palette`): the actions of the
//! instance that has the focus and of every instance around it, innermost first, each under its
//! block's name.

use std::collections::HashSet;
use std::rc::Rc;

use be_sdk_api::{EditorInstanceId, PaletteMode, ViewId};
use beui::icons::ICON_TERMINAL;
use beui::native::try_with_document;
use beui::reactive::*;
use beui::styled::*;
use beui::{Key, KeyAt, NodeId};

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
}

#[component]
pub(crate) fn HostPalette() -> NodeId {
    let open = create_signal(false);
    let rows = create_signal(Rc::new(Vec::<Row>::new()));
    let items = create_memo(move || {
        rows.with(|rows| rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>())
    });
    let show = move |from: Option<NodeId>| {
        rows.set(Rc::new(actions_from(from)));
        open.set(true);
    };
    Action::new("host.palette", "Command palette", move || show(focused_catcher()))
        .glyph(ICON_TERMINAL)
        .shortcut(PALETTE)
        .at(KeyAt::Window)
        .capture()
        .register();
    let any = super::any();
    create_effect(move || {
        any.get();
        untrack(|| {
            let asked = be_host_runtime::take_palette_asks().into_iter().last();
            if let Some(ask) = asked {
                match ask.mode {
                    PaletteMode::Actions | PaletteMode::Blocks => {
                        show(catcher_of(&ask.app, ask.instance));
                    }
                }
            }
        });
    });
    view! {
        <CommandPalette
            open
            items={PaletteItems::list(items)}
            score={PaletteItem::fuzzy_score as PaletteScore}
            id="host.palette"
            on_choose={move |key: String| choose(rows, &key)}
        />
    }
}

fn choose(rows: Signal<Rc<Vec<Row>>>, key: &str) {
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
        None => {}
    }
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
        let mut listed = HashSet::new();
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
    rows
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
