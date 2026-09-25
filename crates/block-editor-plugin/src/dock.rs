use std::cell::Cell;

use beui::NodeId;
use beui::reactive::{
    Callback, Frame, Func, Memo, Prop, RenderFn, component, create_memo, on_cleanup, view,
};
use beui::styled::DockArea;
use beui::unstyled::{DockState, TabId};

use crate::Editor;

#[derive(Clone)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) struct DockLink {
    pub(crate) key: u64,
    pub(crate) state: Memo<DockState>,
    pub(crate) title: Func<TabId, String>,
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
    closable: Option<Func<TabId, bool>>,
    #[prop(children)] content: RenderFn<TabId>,
) -> NodeId {
    let closable = closable.unwrap_or_else(|| Func::new(|_| true));
    if !editor.host().panes_offered() {
        return view! {
            <DockArea
                state
                title
                closable
                content
                on_change={move |next: DockState| on_change.call(next)}
                on_close={move |tab: TabId| on_close.call(tab)}
            />
        };
    }
    let key = NEXT_LINK.with(|next| next.replace(next.get() + 1));
    let host = editor.host().clone();
    host.set_dock(Some(DockLink {
        key,
        state: create_memo(move || state.get()),
        title,
        closable,
        on_change,
        on_close,
        content,
    }));
    on_cleanup(move || host.forget_dock(key));
    view! {
        <Frame />
    }
}
