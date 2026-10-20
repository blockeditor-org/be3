use crate::frame::{ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::theme::BORDER_WIDTH;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{SHEET_STOPS, SheetGripHandle};
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HANDLE_HEIGHT: f32 = 24.0;
const GRIP_WIDTH: f32 = 36.0;
const GRIP_HEIGHT: f32 = 4.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 150);

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = SHEET_STOPS[1])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    view! {
        <unstyled::Sheet
            extent
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            grip={|_: SheetGripHandle| view! {
                <SheetGrip />
            }}
            panel={|content: Child| view! {
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
    on_close: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    view! {
        <unstyled::ModalSheet
            open
            rest
            stops
            fit
            fade={crate::theme::SCROLL_FADE}
            scrim=SCRIM
            grip={|_: SheetGripHandle| view! {
                <SheetGrip />
            }}
            panel={|content: Child| view! {
                <SheetPanel>{content}</SheetPanel>
            }}
            on_close={on_close}
        >|| {
            content.call(())
        }</unstyled::ModalSheet>
    }
}

#[component]
fn SheetPanel(children: Child) -> NodeId {
    view! {
        <Frame fill=ColorRole::Surface>{children}</Frame>
    }
}

#[component]
fn SheetGrip() -> NodeId {
    view! {
        <List spacing=0.0>
            <Frame height=BORDER_WIDTH fill=ColorRole::Border />
            <Frame height=HANDLE_HEIGHT align=Align::Center>
                <Frame
                    width=GRIP_WIDTH
                    height=GRIP_HEIGHT
                    radius=Radius::Xs
                    fill=ColorRole::TextMuted
                />
            </Frame>
        </List>
    }
}
