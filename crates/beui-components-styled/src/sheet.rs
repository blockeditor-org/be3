use beui_macros::{component, view};

use crate::theme::{BORDER_WIDTH, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{SHEET_STOPS, SheetGripHandle};
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::reactive::{Align, Child, ClickCallback, Frame, List, Prop};

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
            grip={|_: SheetGripHandle| view! {
                <SheetGrip />
            }}
            panel={|content: Child| view! {
                <SheetPanel>{content}</SheetPanel>
            }}
            on_close={move || on_close.call()}
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
    children: Child,
) -> NodeId {
    view! {
        <unstyled::ModalSheet
            open
            rest
            stops
            fit
            scrim=SCRIM
            grip={|_: SheetGripHandle| view! {
                <SheetGrip />
            }}
            panel={|content: Child| view! {
                <SheetPanel>{content}</SheetPanel>
            }}
            on_close={move || on_close.call()}
        >
            {children}
        </unstyled::ModalSheet>
    }
}

#[component]
fn SheetPanel(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame color={theme.surface.clone()}>{children}</Frame>
    }
}

#[component]
fn SheetGrip() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <Frame height=BORDER_WIDTH color={theme.border.clone()} />
            <Frame height=HANDLE_HEIGHT align_horizontal=Align::Center align_vertical=Align::Center>
                <Frame
                    width=GRIP_WIDTH
                    height=GRIP_HEIGHT
                    radius=2
                    color={theme.text_muted.clone()}
                />
            </Frame>
        </List>
    }
}
