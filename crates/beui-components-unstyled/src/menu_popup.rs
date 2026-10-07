use std::rc::Rc;

use beui_macros::{component, view};

use crate::menu::{MenuItem, MenuList, MenuRowHandle};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Callback, Child, ClickCallback, List, Memo, NodeRef, Prop, RenderFn, Run, Show, clone,
    create_memo,
};

pub struct MenuSheetHandle {
    pub open: Memo<bool>,
    pub on_close: ClickCallback,
    pub menu: Child,
}

struct MenuParts {
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    sheet: Option<(RenderFn<MenuRowHandle>, RenderFn<MenuSheetHandle>)>,
}

#[derive(Clone, Default)]
pub struct MenuStyle(Option<Rc<MenuParts>>);

impl MenuStyle {
    pub fn new(
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self(Some(Rc::new(MenuParts {
            row: RenderFn::new(row),
            panel: RenderFn::new(panel),
            sheet: None,
        })))
    }

    pub fn with_sheet(
        self,
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        sheet: impl Fn(MenuSheetHandle) -> NodeId + 'static,
    ) -> Self {
        let (dropdown_row, panel) = self.parts();
        Self(Some(Rc::new(MenuParts {
            row: dropdown_row,
            panel,
            sheet: Some((RenderFn::new(row), RenderFn::new(sheet))),
        })))
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn parts(&self) -> (RenderFn<MenuRowHandle>, RenderFn<Child>) {
        match &self.0 {
            Some(parts) => (parts.row.clone(), parts.panel.clone()),
            None => (
                RenderFn::new(|_| {
                    view! {
                        <List spacing=0.0 />
                    }
                }),
                RenderFn::new(|content| content),
            ),
        }
    }

    fn sheet(&self) -> Option<(RenderFn<MenuRowHandle>, RenderFn<MenuSheetHandle>)> {
        self.0.as_ref().and_then(|parts| parts.sheet.clone())
    }
}

#[component]
pub(crate) fn MenuPopup(
    items: Run<MenuItem>,
    menu: MenuStyle,
    anchor: Prop<OverlayAnchor>,
    open: Memo<bool>,
    touched: Memo<bool>,
    #[prop(default = true)] focusing: Prop<bool>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    #[prop(default = NodeRef::new())] content: NodeRef,
    on_dismiss: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let (row, panel) = menu.parts();
    let sheet = menu.sheet();
    let sheeted = sheet.is_some();
    let dropped = create_memo(clone!(open touched -> move || {
        open.get() && !(sheeted && touched.get())
    }));
    let raised = create_memo(clone!(open touched -> move || {
        sheeted && open.get() && touched.get()
    }));
    let traps_focus = focusing.clone();
    let active = create_memo(clone!(dropped -> move || dropped.get() && focusing.get()));
    let choose = Callback::new(clone!(on_dismiss -> move |path: Vec<usize>| {
        on_select.call(path);
        on_dismiss.call();
    }));
    let (sheet_items, sheet_panel, sheet_choose, sheet_dismiss) = (
        items.clone(),
        panel.clone(),
        choose.clone(),
        on_dismiss.clone(),
    );
    view! {
        <List spacing=0.0>
            <Overlay
                @node_ref=&overlay
                anchor
                placement=Placement::BelowStart
                traps_focus
                open={dropped}
                on_dismiss={move || on_dismiss.call()}
            >
                {panel.call(view! {
                    <MenuList
                        @node_ref=&content
                        items
                        row
                        panel={panel.clone()}
                        active
                        on_select={move |path: Vec<usize>| choose.call(path)}
                    />
                })}
            </Overlay>
            <Show condition={sheeted}>
                {move || {
                    let (row, sheet) = sheet.clone().unwrap_or_else(|| unreachable!());
                    let (choose, dismiss) = (sheet_choose.clone(), sheet_dismiss.clone());
                    sheet.call(MenuSheetHandle {
                        open: raised.clone(),
                        on_close: ClickCallback::new(move || dismiss.call()),
                        menu: view! {
                            <MenuList
                                items={sheet_items.clone()}
                                row
                                panel={sheet_panel.clone()}
                                active={raised.clone()}
                                on_select={move |path: Vec<usize>| choose.call(path)}
                            />
                        },
                    })
                }}
            </Show>
        </List>
    }
}
