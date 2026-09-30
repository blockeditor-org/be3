use std::cell::Cell;

use beui::NodeId;
use beui::reactive::{
    Callback, Frame, Func, ItemSize, List, Memo, Prop, RenderFn, Show, clone, component,
    create_effect, create_memo, on_cleanup, untrack, view,
};
use beui::styled::DockArea;
use beui::unstyled::{DockMode, DockState, TabId};

use crate::Editor;

#[derive(Clone)]
pub(crate) struct DockLink {
    pub(crate) key: u64,
    pub(crate) state: Memo<DockState>,
    pub(crate) title: Func<TabId, String>,
    pub(crate) icon: Func<TabId, String>,
    pub(crate) closable: Func<TabId, bool>,
    pub(crate) on_change: Callback<DockState>,
    pub(crate) on_close: Callback<TabId>,
    pub(crate) content: RenderFn<TabId>,
}

thread_local! {
    static NEXT_LINK: Cell<u64> = const { Cell::new(1) };
}

#[component]
pub fn EditorDock(
    editor: Editor,
    state: Prop<DockState>,
    on_change: Callback<DockState>,
    on_close: Callback<TabId>,
    title: Func<TabId, String>,
    icon: Option<Func<TabId, String>>,
    closable: Option<Func<TabId, bool>>,
    #[prop(default = DockMode::Tiled)] mode: Prop<DockMode>,
    #[prop(default = None)] home: Prop<Option<TabId>>,
    empty: Option<RenderFn<()>>,
    #[prop(children)] content: RenderFn<TabId>,
) -> NodeId {
    let closable = closable.unwrap_or_else(|| Func::new(|_| true));
    let icon = icon.unwrap_or_else(|| Func::new(|_| String::new()));
    let empty = empty.unwrap_or_else(|| {
        RenderFn::new(|()| {
            view! {
                <Frame />
            }
        })
    });
    let offered = editor.host().panes_offered();
    let mode = create_memo(move || mode.get());
    let paned = create_memo(clone!(mode -> move || offered && mode.get() == DockMode::Tiled));
    let local = create_memo(clone!(paned -> move || !paned.get()));
    let state = create_memo(move || state.get());
    let home = create_memo(move || home.get());
    if offered {
        let key = NEXT_LINK.with(|next| next.replace(next.get() + 1));
        let link = DockLink {
            key,
            state: state.clone(),
            title: title.clone(),
            icon: icon.clone(),
            closable: closable.clone(),
            on_change: on_change.clone(),
            on_close: on_close.clone(),
            content: content.clone(),
        };
        create_effect(clone!(editor paned -> move || {
            let paned = paned.get();
            untrack(|| match paned {
                true => editor.set_dock(link.clone()),
                false => editor.forget_dock(key),
            });
        }));
        on_cleanup(move || editor.forget_dock(key));
    }
    view! {
        <List spacing=0.0>
            <Show condition={local}>
                <DockArea
                    @sizing=ItemSize::Percent(100.0)
                    state={state.clone()}
                    mode={mode.clone()}
                    home={home.clone()}
                    title={title.clone()}
                    icon={icon.clone()}
                    closable={closable.clone()}
                    content={content.clone()}
                    empty={clone!(empty -> move || empty.call(()))}
                    on_change={clone!(on_change -> move |next: DockState| on_change.call(next))}
                    on_close={clone!(on_close -> move |tab: TabId| on_close.call(tab))}
                />
            </Show>
        </List>
    }
}
