use crate::frame::{ColorRole, Frame, Radius};

use crate::border::Separator;
use crate::focus_ring::FocusRing;
use crate::list::List;
use crate::tokens::{Size, Space, Stroke};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{AnchoredSheetHandle, SheetGripHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Align;

/// Where a sheet stops, as shares of the screen: low, middle and high. A sheet fitted to what
/// it holds rises at most to the middle stop, and past that what it holds scrolls.
pub const SHEET_STOPS: [f32; 3] = [0.3, 0.5, 0.9];
/// The highest a fitted sheet rises: the middle stop.
const FITTED: f32 = SHEET_STOPS[1];

const HANDLE_HEIGHT: Size = Size::Md;
const GRIP_WIDTH: Size = Size::Custom(36.0);
const GRIP_HEIGHT: Size = Size::Custom(4.0);

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
    /// Rests at its own height, never above the middle stop: past that what it holds scrolls.
    /// It then takes no `rest` or `stops` of its own.
    #[prop(default = false)]
    fit: bool,
    on_close: Callback<()>,
    children: NodeId,
) -> NodeId {
    let rest = match fit {
        true => FITTED,
        false => rest,
    };
    view! {
        <unstyled::Sheet
            extent
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            style={crate::scroll::scroll_style()}
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
    /// Rests at its own height, never above the middle stop: past that what it holds scrolls.
    /// It then takes no `rest` or `stops` of its own.
    #[prop(default = false)]
    fit: bool,
    on_close: Callback<()>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> NodeId {
    let theme = crate::theme::use_theme();
    let scrim = create_memo(move || ColorRole::Scrim.resolve(&theme));
    let rest = match fit {
        true => FITTED,
        false => rest,
    };
    view! {
        <unstyled::ModalSheet
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            style={crate::scroll::scroll_style()}
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
    // Filled, as it stays over what the sheet holds while that scrolls under it.
    view! {
        <Frame fill=ColorRole::Surface>
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
        </Frame>
    }
}
