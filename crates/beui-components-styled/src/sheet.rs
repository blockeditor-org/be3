use crate::frame::{ColorRole, Frame, Radius};

use crate::border::Separator;
use crate::focus_ring::FocusRing;
use crate::list::List;
use crate::tokens::{Movement, Size, Space, Stroke};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{AnchoredSheetHandle, SheetCurves, SheetGripHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Align;

pub const SHEET_STOPS: [f32; 3] = [0.3, 0.5, 0.9];

const HANDLE_HEIGHT: Size = Size::Md;
const GRIP_WIDTH: Size = Size::Custom(36.0);
const GRIP_HEIGHT: Size = Size::Custom(4.0);
const ANCHORED_STOPS: [f32; 2] = [0.5, 0.9];
const CURVES: SheetCurves = SheetCurves {
    enter: Movement::Settle.spring(),
    settle: Movement::Settle.spring(),
    pull: Movement::Pull.spring(),
    rubber: Movement::Rubber.spring(),
};

pub fn modal_sheet_room(extent: f32) -> f32 {
    let grip = crate::tokens::Stroke::Hairline.px() + HANDLE_HEIGHT.px();
    (SHEET_STOPS[2] * extent - grip).max(0.0)
}

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = SHEET_STOPS[1])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    on_close: Callback<()>,
    children: NodeId,
) -> NodeId {
    view! {
        <unstyled::Sheet
            extent
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            curves=CURVES
            grip={|handle: SheetGripHandle| view! {
                <SheetGrip focused={handle.focused} />
            }}
            panel={|content: NodeId| view! {
                <SheetPanel>{content}</SheetPanel>
            }}
            on_close={on_close}
        >
            {children}
        </unstyled::Sheet>
    }
}

#[component]
pub fn ModalSheet(
    open: Prop<bool>,
    #[prop(default = SHEET_STOPS[2])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    on_close: Callback<()>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> NodeId {
    let theme = crate::theme::use_theme();
    let scrim = create_memo(move || ColorRole::Scrim.resolve(&theme));
    view! {
        <unstyled::ModalSheet
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            curves=CURVES
            scrim={scrim}
            grip={|handle: SheetGripHandle| view! {
                <SheetGrip focused={handle.focused} />
            }}
            panel={|content: NodeId| view! {
                <SheetPanel>{content}</SheetPanel>
            }}
            on_close={on_close}
        >|| {
            content.call()
        }</unstyled::ModalSheet>
    }
}

#[component]
pub(crate) fn AnchoredSheet(handle: AnchoredSheetHandle, padding: Space) -> NodeId {
    let AnchoredSheetHandle {
        open,
        on_close,
        content,
    } = handle;
    view! {
        <ModalSheet
            open={open}
            rest={ANCHORED_STOPS[0]}
            stops={ANCHORED_STOPS.to_vec()}
            fit=true
            on_close={on_close}
        >||
            <Frame padding>{content.call()}</Frame>
        </ModalSheet>
    }
}

#[component]
fn SheetPanel(children: Child) -> NodeId {
    view! {
        <Frame fill=ColorRole::Surface>{children}</Frame>
    }
}

#[component]
fn SheetGrip(focused: ReadSignal<bool>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Separator />
            <Frame height=HANDLE_HEIGHT align=Align::Center>
                <FocusRing focused offset=Stroke::Focus>
                    <Frame
                        width=GRIP_WIDTH
                        height=GRIP_HEIGHT
                        radius=Radius::Xs
                        fill=ColorRole::TextMuted
                    />
                </FocusRing>
            </Frame>
        </List>
    }
}
