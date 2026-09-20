use beui_macros::{component, view};

use crate::base::{Align, Direction, ItemSize};
use crate::color::Color32;
use crate::icons::{ICON_CLOSE, ICON_DRAG_INDICATOR};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Child, ClickCallback, Frame, Func, List, Memo, Prop, ReadSignal, RenderFn, Show,
    Spacer, Text, clone, create_memo,
};
use crate::styled::button::ButtonVariant;
use crate::styled::context_menu::ContextMenu;
use crate::styled::icon_button::{IconButton, IconButtonSize};
use crate::styled::text::{Body, IconSized};
use crate::styled::theme::{BORDER_WIDTH, CARD_RADIUS, FONT_BODY, RADIUS, use_theme};
use crate::unstyled;
use crate::unstyled::{
    DockPanelHandle, DockSplitterHandle, DockState, DockTabHandle, DockWindowGripHandle,
    DockWindowHandle, MenuItem, TabId,
};

const TAB_PADDING_HORIZONTAL: f32 = 10.0;
const TAB_PADDING_VERTICAL: f32 = 6.0;
const TAB_SPACING: f32 = 6.0;
const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 4.0;
const WINDOW_BAR_PADDING: f32 = 5.0;
const GRIP_WIDTH: f32 = 22.0;
const GRIP_PADDING: f32 = 3.0;
const GRIP_GLYPH: f32 = 16.0;
const PREVIEW_PADDING: f32 = 8.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const DROP_ALPHA: u8 = 64;
const PREVIEW_ALPHA: u8 = 235;

#[component]
pub fn DockArea(
    state: Prop<DockState>,
    on_change: Callback<DockState>,
    on_close: Callback<TabId>,
    title: Func<TabId, String>,
    closable: Option<Func<TabId, bool>>,
    #[prop(children)] content: RenderFn<TabId>,
) -> NodeId {
    let closable = closable.unwrap_or_else(|| Func::new(|_| true));
    let previews = title.clone();
    view! {
        <unstyled::Dock
            state
            on_change={move |state: DockState| on_change.call(state)}
            on_close={move |tab: TabId| on_close.call(tab)}
            title
            content
            tab={move |handle: DockTabHandle| {
                let closable = closable.clone();
                view! {
                    <DockTabFace handle closable />
                }
            }}
            panel={move |handle: DockPanelHandle| view! {
                <DockPanelFace handle />
            }}
            splitter={move |handle: DockSplitterHandle| view! {
                <DockSplitterFace handle />
            }}
            window_grip={move |handle: DockWindowGripHandle| view! {
                <DockWindowGrip handle />
            }}
            window={move |handle: DockWindowHandle| view! {
                <DockWindowFace handle />
            }}
            highlight={move || view! {
                <DockDropHighlight />
            }}
            preview={move |tab: TabId| {
                let titles = previews.clone();
                let title = create_memo(move || titles.call(tab));
                view! {
                    <DockDragPreview title />
                }
            }}
        />
    }
}

#[component]
fn DockTabFace(handle: DockTabHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockTabHandle {
        tab,
        title,
        selected,
        hovered,
        active,
        focused,
        dragged,
        close,
        float,
        floating,
        ..
    } = handle;
    let closable = closable.call(tab);
    let closing = close.clone();
    view! {
        <ContextMenu
            items={view! {
                <MenuItem label="Pop out into a window" disabled={floating} />
                <MenuItem label="Close tab" disabled={!closable} />
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => float.call(),
                Some(1) => closing.call(),
                _ => {}
            }}
        >
            <DockTabChrome
                title
                selected
                hovered
                active
                focused
                dragged
                closable
                close={move || close.call()}
            />
        </ContextMenu>
    }
}

#[component]
fn DockTabChrome(
    title: Prop<String>,
    selected: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    dragged: Memo<bool>,
    closable: bool,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected hovered -> move || {
        match (selected.get(), hovered.get() || active.get(), dragged.get()) {
            (_, _, true) => theme.accent_soft.get(),
            (true, _, false) => theme.surface.get(),
            (false, true, false) => theme.hover.get(),
            (false, false, false) => theme.background.get(),
        }
    }));
    let label = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.text.get(),
        false => theme.text_muted.get(),
    }));
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            padding_horizontal=TAB_PADDING_HORIZONTAL
            padding_vertical=TAB_PADDING_VERTICAL
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_offset=1.0
            outline_visible={focused}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Text string={title} font_size=FONT_BODY color={label} clip=true />
                <Show condition={closable}>
                    <IconButton
                        glyph=ICON_CLOSE
                        label="Close tab"
                        size=IconButtonSize::Compact
                        variant=ButtonVariant::Ghost
                        capture_presses=true
                        on_click={move || close.call()}
                    />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn DockPanelFace(handle: DockPanelHandle) -> NodeId {
    let DockPanelHandle {
        floating,
        focused,
        bar,
        body,
        ..
    } = handle;
    let theme = use_theme();
    let outline = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={outline}
            outline_width=BORDER_WIDTH
            outline_visible={!floating}
            radius=RADIUS
        >
            <List spacing=0.0>
                <Show condition={bar.is_some()}>
                    <DockBarRow bar={bar.unwrap_or_else(|| unreachable!())} />
                </Show>
                {body} @sizing=ItemSize::Percent(100.0)
            </List>
        </Frame>
    }
}

#[component]
fn DockBarRow(bar: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=BAR_PADDING
            padding_vertical=BAR_PADDING
        >
            {bar}
        </Frame>
    }
}

#[component]
fn DockSplitterFace(handle: DockSplitterHandle) -> NodeId {
    let DockSplitterHandle {
        hovered,
        active,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(
        clone!(theme -> move || match (active.get(), hovered.get()) {
            (true, _) => theme.accent.get(),
            (false, true) => theme.border.get(),
            (false, false) => theme.background.get(),
        }),
    );
    view! {
        <Frame
            color={fill}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_visible={focused}
        />
    }
}

#[component]
fn DockWindowGrip(handle: DockWindowGripHandle) -> NodeId {
    let DockWindowGripHandle { focused, .. } = handle;
    let theme = use_theme();
    let color = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.text_muted.get(),
        false => theme.border.get(),
    }));
    view! {
        <Frame width=GRIP_WIDTH padding_vertical=WINDOW_BAR_PADDING padding_horizontal=GRIP_PADDING>
            <IconSized glyph=ICON_DRAG_INDICATOR font_size=GRIP_GLYPH color={color} />
        </Frame>
    }
}

#[component]
fn DockWindowFace(handle: DockWindowHandle) -> NodeId {
    let DockWindowHandle {
        focused,
        title,
        grip,
        tabs,
        close,
        pane,
        ..
    } = handle;
    let theme = use_theme();
    let outline = create_memo(clone!(theme focused -> move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let fill = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.surface_raised.get(),
        false => theme.surface.get(),
    }));
    let titled = tabs.is_none();
    view! {
        <Frame
            color={theme.background.clone()}
            outline={outline}
            outline_width=FOCUS_RING_WIDTH
            outline_visible=true
            radius=CARD_RADIUS
        >
            <List spacing=0.0>
                <Frame color={fill} padding_vertical=WINDOW_BAR_PADDING>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=BAR_SPACING
                    >
                        {grip}
                        <Show condition={!titled}>
                            {tabs.unwrap_or_else(|| unreachable!())}
                        </Show>
                        <Show condition={titled}>
                            <Body content={title} />
                        </Show>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Frame padding_horizontal=WINDOW_BAR_PADDING>
                            <IconButton
                                glyph=ICON_CLOSE
                                label="Close window"
                                size=IconButtonSize::Compact
                                variant=ButtonVariant::Ghost
                                capture_presses=true
                                on_click={move || close.call()}
                            />
                        </Frame>
                    </List>
                </Frame>
                {pane} @sizing=ItemSize::Percent(100.0)
            </List>
        </Frame>
    }
}

#[component]
fn DockDropHighlight() -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || translucent(theme.accent.get(), DROP_ALPHA)));
    view! {
        <Frame
            color={fill}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_visible=true
            radius=RADIUS
        />
    }
}

#[component]
fn DockDragPreview(title: Prop<String>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(
        clone!(theme -> move || translucent(theme.surface_raised.get(), PREVIEW_ALPHA)),
    );
    view! {
        <Frame
            color={fill}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_visible=true
            radius=RADIUS
            padding_horizontal=PREVIEW_PADDING
            padding_vertical=TAB_PADDING_VERTICAL
        >
            <Body content={title} />
        </Frame>
    }
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}
