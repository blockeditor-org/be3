mod stack;

use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::context_menu::menu_style;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::menu_button::IconMenuButton;
use crate::text::{Body, IconSized};
use crate::theme::{CARD_RADIUS, FOCUS_RING_WIDTH, FONT_BODY, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    DockBarHandle, DockChromeHandle, DockDragged, DockGripHandle, DockKey, DockMode, DockNode,
    DockPreviewHandle, DockSplitterHandle, DockStackHandle, DockSwitcherHandle, DockTabHandle,
    DockingLayout, Entry, MenuItem,
};
use beui_core::base::{Align, Direction, ItemSize, Justify};
use beui_core::color::Color32;
use beui_core::icons::{ICON_CLOSE, ICON_DRAG_INDICATOR, ICON_MORE_VERT, ICON_TAB_GROUP};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Children, ClickCallback, DynamicSegment, ForEach, Frame, List, ListChild, Memo, Prop,
    ReadSignal, Show, Text, clone, create_memo, focus_ring,
};
use stack::{DockStackBar, DockSwitcher};

const TAB_PADDING_HORIZONTAL: f32 = 10.0;
const TAB_HEIGHT: f32 = 33.0;
const TAB_SPACING: f32 = 6.0;
const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 4.0;
const WINDOW_BAR_PADDING: f32 = 5.0;
const GRIP_WIDTH: f32 = 22.0;
const GRIP_PADDING: f32 = 3.0;
const GRIP_GLYPH: f32 = 16.0;
const PREVIEW_PADDING: f32 = 8.0;
pub const CHROME_BORDER: f32 = 2.0;
const GROUP_GLYPH: f32 = 16.0;
const GROUP_INSET: f32 = 6.0;
pub const DOCK_INSET: f32 = 8.0;
pub const WINDOW_CHROME: beui_core::geometry::Vec2 = beui_core::geometry::Vec2::new(
    2.0 * CHROME_BORDER,
    TAB_HEIGHT + 2.0 * WINDOW_BAR_PADDING + CHROME_BORDER,
);
const DROP_ALPHA: u8 = 64;
const PREVIEW_ALPHA: u8 = 235;

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = DockMode::Tiled)] mode: Prop<DockMode>,
    #[prop(default = None)] home: Prop<Option<K>>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let mode = create_memo(move || mode.get());
    let padding = create_memo(clone!(mode -> move || match mode.get() {
        DockMode::Tiled => DOCK_INSET,
        DockMode::Stacked => 0.0,
    }));
    let theme = use_theme();
    let backdrop = theme.background.clone();
    let fill = create_memo(clone!(mode -> move || match mode.get() {
        DockMode::Tiled => Color32::TRANSPARENT,
        DockMode::Stacked => theme.background.get(),
    }));
    view! {
        <unstyled::Docking
            layout
            mode
            home
            focus
            inset={padding}
            group_inset=GROUP_INSET
            menu={menu_style()}
            children
            frame={move |body: NodeId| view! {
                <Frame color={fill.clone()}>{body}</Frame>
            }}
            fullscreen={move |body: NodeId| view! {
                <Frame color={backdrop.clone()}>{body}</Frame>
            }}
            stack={move |handle: DockStackHandle| view! {
                <DockStackBar handle />
            }}
            switcher={move |handle: DockSwitcherHandle| view! {
                <DockSwitcher handle />
            }}
            tab={move |handle: DockTabHandle| view! {
                <DockTabFace handle />
            }}
            chrome={move |handle: DockChromeHandle| view! {
                <DockChromeFace handle />
            }}
            bar={move |handle: DockBarHandle| view! {
                <DockBarFace handle />
            }}
            splitter={move |handle: DockSplitterHandle| view! {
                <DockSplitterFace handle />
            }}
            grip={move |handle: DockGripHandle| view! {
                <DockGrip handle />
            }}
            highlight={move || view! {
                <DockDropHighlight />
            }}
            preview={move |handle: DockPreviewHandle| {
                let DockPreviewHandle { dragged, title, icon } = handle;
                let grouped = !matches!(dragged, DockDragged::Entry(Entry::Tab(_)));
                view! {
                    <DockDragPreview title icon grouped />
                }
            }}
        />
    }
}

#[component]
fn DockTabFace(handle: DockTabHandle) -> NodeId {
    let DockTabHandle {
        entry,
        title,
        icon,
        selected,
        hovered,
        active,
        focused,
        dragged,
        closable,
        close,
        vertical,
        ..
    } = handle;
    view! {
        <DockTabChrome
            title
            icon
            grouped={matches!(entry, Entry::Group(_))}
            vertical
            selected
            hovered
            active
            focused
            dragged
            closable
            close_test_id={close_test_id(entry)}
            close={move || close.call()}
        />
    }
}

#[component]
fn DockTabChrome(
    title: Prop<String>,
    icon: Memo<String>,
    grouped: bool,
    vertical: bool,
    selected: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    dragged: Memo<bool>,
    closable: Memo<bool>,
    close_test_id: String,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected hovered -> move || {
        match (selected.get(), hovered.get() || active.get(), dragged.get()) {
            (_, _, true) => theme.accent_soft.get(),
            (true, _, false) => theme.background.get(),
            (false, true, false) => theme.hover.get(),
            (false, false, false) => theme.surface.get(),
        }
    }));
    let label = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.text.get(),
        false => theme.text_muted.get(),
    }));
    let glyph = label.clone();
    let pictured = create_memo(clone!(icon -> move || !grouped && !icon.get().is_empty()));
    let icon_color = label.clone();
    let title_size = match vertical {
        true => ItemSize::Percent(100.0),
        false => ItemSize::Intrinsic,
    };
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            height=TAB_HEIGHT
            padding_horizontal=TAB_PADDING_HORIZONTAL
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_offset=1.0
            outline_visible={focus_ring(focused)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <IconSized glyph=ICON_TAB_GROUP font_size=GROUP_GLYPH color={glyph.clone()} />
                </Show>
                <Show condition={pictured}>
                    <IconSized
                        glyph={icon.clone()}
                        font_size=GROUP_GLYPH
                        color={icon_color.clone()}
                    />
                </Show>
                <Text
                    string={title}
                    font_size=FONT_BODY
                    color={label}
                    clip=true
                    @sizing={title_size}
                />
                <Show condition={closable}>
                    {move || clone!(close -> view! {
                        <IconButton
                            @test_id={close_test_id.clone()}
                            glyph=ICON_CLOSE
                            label="Close tab"
                            size=IconButtonSize::Compact
                            variant=ButtonVariant::Ghost
                            capture_presses=true
                            on_click={move || close.call()}
                        />
                    })}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn DockChromeFace(handle: DockChromeHandle) -> NodeId {
    let DockChromeHandle {
        barred,
        focused,
        content,
        ..
    } = handle;
    let theme = use_theme();
    let outline = create_memo(clone!(theme focused -> move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let border = match barred {
        true => CHROME_BORDER,
        false => 0.0,
    };
    view! {
        <Frame
            color={theme.background.clone()}
            outline={outline}
            outline_width=CHROME_BORDER
            outline_visible=barred
            radius={if barred { CARD_RADIUS } else { 0 }}
            padding_horizontal=border
            padding_vertical=border
        >
            {content}
        </Frame>
    }
}

#[component]
fn DockBarFace(handle: DockBarHandle) -> NodeId {
    let DockBarHandle {
        vertical,
        grip,
        tabs,
        title,
        closable,
        menu,
        close,
        ..
    } = handle;
    let close = move || close.call();
    match vertical {
        true => view! {
            <DockSideBar grip tabs title menu closable close />
        },
        false => view! {
            <DockTitleBar grip tabs title menu closable close />
        },
    }
}

#[component]
fn DockTitleBar(
    grip: NodeId,
    tabs: Option<NodeId>,
    title: Prop<String>,
    menu: Memo<Vec<Action>>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let titled = tabs.is_none();
    view! {
        <Frame
            color={theme.surface.clone()}
            padding_vertical=BAR_PADDING
            padding_horizontal=BAR_PADDING
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                {grip}
                {tabs} @sizing=ItemSize::Percent(100.0)
                <Show condition={titled}>
                    <Body content={title.clone()} @sizing=ItemSize::Percent(100.0) />
                </Show>
                <DockMenu menu />
                <DockClose closable close={move || close.call()} />
            </List>
        </Frame>
    }
}

#[component]
fn DockSideBar(
    grip: NodeId,
    tabs: Option<NodeId>,
    title: Prop<String>,
    menu: Memo<Vec<Action>>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let titled = tabs.is_none();
    view! {
        <Frame
            color={theme.surface.clone()}
            padding_vertical=BAR_PADDING
            padding_horizontal=BAR_PADDING
        >
            <List spacing=BAR_SPACING>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::SpaceBetween
                    spacing=BAR_SPACING
                >
                    {grip}
                    <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                        <DockMenu menu />
                        <DockClose closable close={move || close.call()} />
                    </List>
                </List>
                {tabs} @sizing=ItemSize::Percent(100.0)
                <Show condition={titled}>
                    <Body content={title.clone()} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn DockMenu(
    menu: Memo<Vec<Action>>,
    #[prop(default = IconButtonSize::Compact)] size: IconButtonSize,
) -> DynamicSegment<ListChild> {
    let offered = create_memo(clone!(menu -> move || menu.with(|items| !items.is_empty())));
    let keys = create_memo(clone!(menu -> move || {
        menu.with(|items| items.iter().map(Action::key).collect::<Vec<u64>>())
    }));
    view! {
        <Show condition={offered}>
            {move || clone!(menu -> view! {
                <IconMenuButton
                    @test_id={"dock.menu"}
                    label="More"
                    glyph=ICON_MORE_VERT
                    size
                    items={view! {
                        <ForEach keys={keys.clone()}>
                            {move |key: u64| {
                                let item = menu.with_untracked(|items| {
                                    items.iter().find(|action| action.key() == key).cloned()
                                });
                                match item {
                                    Some(action) => view! {
                                        <MenuItem action />
                                    },
                                    None => view! {
                                        <MenuItem label="" disabled=true />
                                    },
                                }
                            }}
                        </ForEach>
                    }}
                />
            })}
        </Show>
    }
}

#[component]
fn DockClose(closable: Memo<bool>, close: ClickCallback) -> NodeId {
    view! {
        <Frame padding_horizontal=WINDOW_BAR_PADDING visible={closable}>
            <IconButton
                glyph=ICON_CLOSE
                label="Close pane"
                size=IconButtonSize::Compact
                variant=ButtonVariant::Ghost
                capture_presses=true
                on_click={move || close.call()}
            />
        </Frame>
    }
}

#[component]
fn DockGrip(handle: DockGripHandle) -> NodeId {
    let DockGripHandle { focused, .. } = handle;
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
            outline_visible={focus_ring(focused)}
        />
    }
}

#[component]
fn DockDropHighlight() -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || translucent(theme.accent.get(), DROP_ALPHA)));
    view! {
        <Frame
            @test_id={"dock.drop"}
            color={fill}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_visible=true
            radius=RADIUS
        />
    }
}

#[component]
fn DockDragPreview(title: Prop<String>, icon: Memo<String>, grouped: bool) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(
        clone!(theme -> move || translucent(theme.surface_raised.get(), PREVIEW_ALPHA)),
    );
    let pictured = create_memo(clone!(icon -> move || !grouped && !icon.get().is_empty()));
    let pictured_color = theme.text.clone();
    view! {
        <Frame
            color={fill}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_visible=true
            radius=RADIUS
            height=TAB_HEIGHT
            padding_horizontal=PREVIEW_PADDING
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <IconSized
                        glyph=ICON_TAB_GROUP
                        font_size=GROUP_GLYPH
                        color={theme.text.clone()}
                    />
                </Show>
                <Show condition={pictured}>
                    <IconSized
                        glyph={icon.clone()}
                        font_size=GROUP_GLYPH
                        color={pictured_color.clone()}
                    />
                </Show>
                <Body content={title} />
            </List>
        </Frame>
    }
}

fn close_test_id(entry: Entry) -> String {
    match entry {
        Entry::Tab(tab) => format!("dock.tab.{}.close", tab.value()),
        Entry::Group(_) => String::new(),
    }
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}
