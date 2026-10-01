use std::cell::Cell;

use beui::NodeId;
use beui::reactive::{
    Callback, Frame, Func, ItemSize, List, Memo, Prop, ReadSignal, RenderFn, Show, WriteSignal,
    clone, component, create_effect, create_memo, create_signal, on_cleanup, view,
};
use beui::styled::DockArea;
use beui::unstyled::{DockMode, DockMores, DockState, TabId};

use crate::Editor;

#[derive(Clone)]
pub(crate) struct DockLink {
    pub(crate) key: u64,
    pub(crate) state: Memo<DockState>,
    pub(crate) home: Memo<Option<TabId>>,
    pub(crate) title: Func<TabId, String>,
    pub(crate) icon: Func<TabId, String>,
    pub(crate) closable: Func<TabId, bool>,
    pub(crate) mores: ReadSignal<DockMores>,
    pub(crate) set_more: WriteSignal<DockMores>,
    pub(crate) on_change: Callback<DockState>,
    pub(crate) on_close: Callback<TabId>,
    pub(crate) content: RenderFn<TabId>,
    pub(crate) empty: RenderFn<()>,
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
    let local = !editor.host().panes_offered();
    if !local {
        let key = NEXT_LINK.with(|next| next.replace(next.get() + 1));
        let (mores, set_more) = create_signal(DockMores::new());
        let waking = editor.clone();
        create_effect(clone!(mores -> move || {
            mores.with(|_| ());
            waking.host().waker().wake();
        }));
        editor.set_dock(DockLink {
            key,
            state: create_memo(clone!(state -> move || state.get())),
            home: create_memo(clone!(home -> move || home.get())),
            title: title.clone(),
            icon: icon.clone(),
            closable: closable.clone(),
            mores,
            set_more,
            on_change: on_change.clone(),
            on_close: on_close.clone(),
            content: content.clone(),
            empty: empty.clone(),
        });
        on_cleanup(move || editor.forget_dock(key));
    }
    view! {
        <List spacing=0.0>
            <Show condition={local}>
                {move || clone!(empty on_change on_close -> view! {
                    <DockArea
                        @sizing=ItemSize::Percent(100.0)
                        state={state.clone()}
                        mode={mode.clone()}
                        home={home.clone()}
                        title={title.clone()}
                        icon={icon.clone()}
                        closable={closable.clone()}
                        content={content.clone()}
                        empty={move || empty.call(())}
                        on_change={move |next: DockState| on_change.call(next)}
                        on_close={move |tab: TabId| on_close.call(tab)}
                    />
                })}
            </Show>
        </List>
    }
}
