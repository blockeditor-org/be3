use beui_macros::{component, view};

use crate::anchored::{Anchored, AnchoredHandle, AnchoredSheetHandle, AnchoredStyle, SheetWhen};
use crate::button::ButtonHandle;
use crate::menu::{MenuItem, MenuList, MenuRowHandle, MenuThen};
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Placement;
use beui_view::types::Rect;

#[derive(Clone, Copy)]
pub struct MenuStyle {
    spacing: f32,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    sheet: Option<(RenderFn<MenuRowHandle>, RenderFn<AnchoredSheetHandle>)>,
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
        sheet: impl Fn(AnchoredSheetHandle) -> NodeId + 'static,
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

    fn sheet(&self) -> Option<(RenderFn<MenuRowHandle>, RenderFn<AnchoredSheetHandle>)> {
        self.sheet
    }
}

#[component]
pub fn MenuPopup(
    items: Run<MenuItem>,
    menu: MenuStyle,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    open: Prop<bool>,
    #[prop(default = SheetWhen::Touched)] sheet: Prop<SheetWhen>,
    #[prop(default = true)] focusing: Prop<bool>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    #[prop(default = NodeRef::new())] content: NodeRef,
    trigger: Option<NodeRef>,
    on_dismiss: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let (row, panel) = menu.parts();
    let spacing = menu.spacing();
    let bounds = create_memo(move || bounds.get());
    let (sheet_row, style, sheet) = match menu.sheet() {
        Some((sheet_row, sheet_panel)) => (
            sheet_row,
            AnchoredStyle {
                dropdown: panel,
                sheet: sheet_panel,
            },
            sheet,
        ),
        None => (
            row,
            AnchoredStyle {
                dropdown: panel,
                sheet: RenderFn::new(|handle: AnchoredSheetHandle| handle.content.call(())),
            },
            Prop::Static(SheetWhen::Never),
        ),
    };
    let choose = Callback::new(move |(path, then): (Vec<usize>, MenuThen)| {
        on_select.call(path);
        if then == MenuThen::Close {
            on_dismiss.call();
        }
    });
    view! {
        <Anchored
            style
            anchor
            placement
            bounds
            open
            sheet
            traps_focus={focusing}
            trigger
            overlay
            on_dismiss={on_dismiss}
        >|handle: AnchoredHandle| {
            let items = items.clone();
            let shown = handle.shown;
            match handle.sheet {
                false => view! {
                    <MenuList
                        @node_ref=&content
                        items
                        spacing
                        row
                        panel
                        active={create_memo(move || shown.get() && focusing.get())}
                        bounds
                        on_select={choose}
                    />
                },
                true => view! {
                    <MenuList
                        items
                        spacing
                        row={sheet_row}
                        panel
                        active={shown}
                        pages=true
                        on_select={choose}
                    />
                },
            }
        }</Anchored>
    }
}

#[component]
pub fn MenuRows(
    items: Run<MenuItem>,
    menu: MenuStyle,
    #[prop(default = false)] active: Prop<bool>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let (row, panel) = menu.parts();
    let spacing = menu.spacing();
    let list = view! {
        <MenuList
            items
            spacing
            row
            panel
            active
            on_select={move |(path, _): (Vec<usize>, MenuThen)| on_select.call(path)}
        />
    };
    panel.call(list)
}
