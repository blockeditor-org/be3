use beui_core::layer::Dismiss;

use beui_macros::{component, view};

use crate::button::ButtonHandle;
use crate::menu::{MenuItem, MenuList, MenuRowHandle};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct MenuSheetHandle {
    pub open: Memo<bool>,
    pub on_close: ClickCallback,
    pub menu: Child,
}

#[derive(Clone, Copy)]
struct MenuParts {
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    sheet: Option<(RenderFn<MenuRowHandle>, RenderFn<MenuSheetHandle>)>,
    toolbar: Option<(RenderFn<ButtonHandle>, RenderFn<Child>)>,
}

#[derive(Clone, Copy, Default)]
pub struct MenuStyle(Option<MenuParts>);

impl MenuStyle {
    pub fn new(
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self(Some(MenuParts {
            row: RenderFn::new(row),
            panel: RenderFn::new(panel),
            sheet: None,
            toolbar: None,
        }))
    }

    pub fn with_sheet(
        self,
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        sheet: impl Fn(MenuSheetHandle) -> NodeId + 'static,
    ) -> Self {
        let (dropdown_row, panel) = self.parts();
        Self(Some(MenuParts {
            row: dropdown_row,
            panel,
            sheet: Some((RenderFn::new(row), RenderFn::new(sheet))),
            toolbar: self.toolbar(),
        }))
    }

    pub fn with_toolbar(
        self,
        button: impl Fn(ButtonHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        let (row, menu_panel) = self.parts();
        Self(Some(MenuParts {
            row,
            panel: menu_panel,
            sheet: self.sheet(),
            toolbar: Some((RenderFn::new(button), RenderFn::new(panel))),
        }))
    }

    pub(crate) fn toolbar(&self) -> Option<(RenderFn<ButtonHandle>, RenderFn<Child>)> {
        self.0.as_ref().and_then(|parts| parts.toolbar)
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn parts(&self) -> (RenderFn<MenuRowHandle>, RenderFn<Child>) {
        match &self.0 {
            Some(parts) => (parts.row, parts.panel),
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
        self.0.as_ref().and_then(|parts| parts.sheet)
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
    let dropped = create_memo(move || open.get() && !(sheeted && touched.get()));
    let raised = create_memo(move || sheeted && open.get() && touched.get());
    let mut ever_raised = false;
    let sheet_built = create_memo(move || {
        ever_raised |= raised.get();
        ever_raised
    });
    let active = create_memo(move || dropped.get() && focusing.get());
    let choose = Callback::new(move |path: Vec<usize>| {
        on_select.call(path);
        on_dismiss.call();
    });
    let (sheet_items, sheet_panel, sheet_choose, sheet_dismiss) =
        (items.clone(), panel, choose, on_dismiss);
    view! {
        <List spacing=0.0>
            <Overlay
                @node_ref=&overlay
                anchor
                placement=Placement::BelowStart
                traps_focus={focusing}
                open={dropped}
                on_dismiss={move |_: Dismiss| on_dismiss.call()}
                content={move || panel.call(view! {
                    <MenuList
                        @node_ref=&content
                        items
                        row
                        panel={panel}
                        active
                        on_select={choose}
                    />
                })}
            />
            <Show condition={sheet_built}>|| {
                let (row, sheet) = sheet.unwrap_or_else(|| unreachable!());
                sheet.call(MenuSheetHandle {
                    open: raised,
                    on_close: ClickCallback::new(move || sheet_dismiss.call()),
                    menu: view! {
                        <MenuList
                            items={sheet_items.clone()}
                            row
                            panel={sheet_panel}
                            active={raised}
                            on_select={sheet_choose}
                        />
                    },
                })
            }</Show>
        </List>
    }
}
