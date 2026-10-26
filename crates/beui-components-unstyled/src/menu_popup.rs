use beui_core::layer::Dismiss;

use beui_macros::{component, view};

use crate::button::ButtonHandle;
use crate::menu::{MenuItem, MenuList, MenuRowHandle, MenuThen};
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
pub struct MenuStyle {
    spacing: f32,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    sheet: Option<(RenderFn<MenuRowHandle>, RenderFn<MenuSheetHandle>)>,
    toolbar: Option<(RenderFn<ButtonHandle>, RenderFn<Child>)>,
}

impl MenuStyle {
    pub fn new(
        spacing: f32,
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self {
            spacing,
            row: RenderFn::new(row),
            panel: RenderFn::new(panel),
            sheet: None,
            toolbar: None,
        }
    }

    pub fn with_sheet(
        self,
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        sheet: impl Fn(MenuSheetHandle) -> NodeId + 'static,
    ) -> Self {
        Self {
            sheet: Some((RenderFn::new(row), RenderFn::new(sheet))),
            ..self
        }
    }

    pub fn with_toolbar(
        self,
        button: impl Fn(ButtonHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self {
            toolbar: Some((RenderFn::new(button), RenderFn::new(panel))),
            ..self
        }
    }

    pub(crate) fn toolbar(&self) -> Option<(RenderFn<ButtonHandle>, RenderFn<Child>)> {
        self.toolbar
    }

    pub(crate) fn spacing(&self) -> f32 {
        self.spacing
    }

    pub(crate) fn parts(&self) -> (RenderFn<MenuRowHandle>, RenderFn<Child>) {
        (self.row, self.panel)
    }

    fn sheet(&self) -> Option<(RenderFn<MenuRowHandle>, RenderFn<MenuSheetHandle>)> {
        self.sheet
    }
}

#[component]
pub fn MenuPopup(
    items: Run<MenuItem>,
    menu: MenuStyle,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    open: Memo<bool>,
    touched: Memo<bool>,
    #[prop(default = true)] focusing: Prop<bool>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    #[prop(default = NodeRef::new())] content: NodeRef,
    on_dismiss: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let (row, panel) = menu.parts();
    let spacing = menu.spacing();
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
    let choose = Callback::new(move |(path, then): (Vec<usize>, MenuThen)| {
        on_select.call(path);
        if then == MenuThen::Close {
            on_dismiss.call();
        }
    });
    let (sheet_items, sheet_panel, sheet_choose, sheet_dismiss) =
        (items.clone(), panel, choose, on_dismiss);
    view! {
        <List spacing=0.0>
            <Overlay
                @node_ref=&overlay
                anchor
                placement
                traps_focus={focusing}
                open={dropped}
                on_dismiss={move |_: Dismiss| on_dismiss.call()}
                content={move || {
                    let items = items.clone();
                    panel.call(view! {
                        <MenuList
                            @node_ref=&content
                            items
                            row
                            panel={panel}
                            spacing
                            active
                            on_select={choose}
                        />
                    })
                }}
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
                            spacing
                            active={raised}
                            pages=true
                            on_select={sheet_choose}
                        />
                    },
                })
            }</Show>
        </List>
    }
}
