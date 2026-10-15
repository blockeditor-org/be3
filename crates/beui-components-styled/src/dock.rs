mod stack;
mod switch;

use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::context_menu::menu_style;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::menu_button::IconMenuButton;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{CARD_RADIUS, FOCUS_RING_WIDTH, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    DockAwayHandle, DockBarHandle, DockChromeHandle, DockDialogHandle, DockDragged, DockGripHandle,
    DockKey, DockMode, DockNode, DockPreviewHandle, DockSplitterHandle, DockStackHandle,
    DockSwitchHandle, DockSwitcherHandle, DockTabHandle, DockingLayout, Entry, MenuItem, Picture,
};
use beui_core::base::{Align, Direction, ItemSize, Justify};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::icons::{
    ICON_CLOSE, ICON_DRAG_INDICATOR, ICON_FULLSCREEN, ICON_FULLSCREEN_EXIT, ICON_MORE_VERT,
    ICON_TAB_GROUP,
};
use beui_core::image::Image;
use beui_core::input::Modifiers;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;
use stack::{DockStackBar, DockSwitcher};
use switch::DockSwitchPanel;

const TAB_PADDING_HORIZONTAL: f32 = 10.0;
const TAB_HEIGHT: f32 = 33.0;
const TAB_SPACING: f32 = 6.0;
const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 4.0;
const CONTROL_PADDING: f32 = 5.0;
const GRIP_WIDTH: f32 = 22.0;
const GRIP_PADDING: f32 = 3.0;
const PREVIEW_PADDING: f32 = 8.0;
pub const CHROME_BORDER: f32 = 2.0;
const GROUP_GLYPH: f32 = 16.0;
const GROUP_INSET: f32 = 6.0;
pub const DOCK_INSET: f32 = 8.0;
pub const WINDOW_CHROME: beui_core::geometry::Vec2 = beui_core::geometry::Vec2::new(
    2.0 * CHROME_BORDER,
    TAB_HEIGHT + 2.0 * BAR_PADDING + 3.0 * CHROME_BORDER,
);
const DROP_ALPHA: u8 = 64;
const AWAY_SPACING: f32 = 12.0;
const AWAY_PADDING: f32 = 24.0;
const PREVIEW_ALPHA: u8 = 235;

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = DockMode::Tiled)] mode: Prop<DockMode>,
    #[prop(default = None)] home: Prop<Option<K>>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let mode = create_memo(move || mode.get());
    let padding = create_memo(move || match mode.get() {
        DockMode::Tiled => DOCK_INSET,
        DockMode::Stacked => 0.0,
    });
    let theme = use_theme();
    let backdrop = theme.background;
    let fill = create_memo(move || match mode.get() {
        DockMode::Tiled => Color32::TRANSPARENT,
        DockMode::Stacked => theme.background.get(),
    });
    view! {
        <unstyled::Docking
            layout
            mode
            home
            focus
            drag_modifier
            inset={padding}
            group_inset=GROUP_INSET
            menu={menu_style()}
            children
            frame={move |body: NodeId| view! {
                <Frame fill=fill>{body}</Frame>
            }}
            fullscreen={move |body: NodeId| view! {
                <Frame fill=backdrop>{body}</Frame>
            }}
            away={move |handle: DockAwayHandle| view! {
                <DockAwayFace handle />
            }}
            stack={move |handle: DockStackHandle| view! {
                <DockStackBar handle />
            }}
            switcher={move |handle: DockSwitcherHandle| view! {
                <DockSwitcher handle />
            }}
            switch={move |handle: DockSwitchHandle| view! {
                <DockSwitchPanel handle />
            }}
            dialog_chrome={move |handle: DockDialogHandle| view! {
                <DockDialogFace handle />
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
fn DockAwayFace(handle: DockAwayHandle) -> NodeId {
    let DockAwayHandle { title, leave, .. } = handle;
    let theme = use_theme();
    let message = create_memo(move || format!("{} is full screen", title.get()));
    view! {
        <Frame
            fill={theme.background}
            align=Align::Center
            padding=AWAY_PADDING
            @test_id="dock.away"
        >
            <List align=Align::Center spacing=AWAY_SPACING>
                <Text size=TextSize::Xl3 color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_FULLSCREEN />
                </Text>
                <Text string={message} align=TextAlign::Center />
                <Button
                    label="Leave full screen"
                    glyph=ICON_FULLSCREEN_EXIT
                    variant=ButtonVariant::Secondary
                    @test_id="dock.away.leave"
                    on_click={move || leave.call()}
                />
            </List>
        </Frame>
    }
}

#[component]
fn DockTabFace(handle: DockTabHandle) -> NodeId {
    let DockTabHandle {
        entry,
        title,
        icon,
        picture,
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
            picture
            grouped={matches!(entry, Entry::Group(_))}
            vertical
            selected
            hovered
            active
            focused
            dragged
            closable
            close_test_id={close_test_id(entry)}
            close={close}
        />
    }
}

#[component]
fn DockTabChrome(
    title: Prop<String>,
    icon: Memo<String>,
    picture: Memo<Option<Image>>,
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
    let fill =
        create_memo(
            move || match (selected.get(), hovered.get() || active.get(), dragged.get()) {
                (_, _, true) => theme.accent_soft.get(),
                (true, _, false) => theme.background.get(),
                (false, true, false) => theme.hover.get(),
                (false, false, false) => theme.surface.get(),
            },
        );
    let label = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let glyph = label;
    let shows_picture = create_memo(move || !grouped && picture.with(Option::is_some));
    let pictured = create_memo(move || !grouped && !shows_picture.get() && !icon.get().is_empty());
    let icon_color = label;
    let title_size = match vertical {
        true => ItemSize::Percent(100.0),
        false => ItemSize::Intrinsic,
    };
    view! {
        <Frame
            fill=fill
            radius=RADIUS
            height=TAB_HEIGHT
            padding={Sides::horizontal(TAB_PADDING_HORIZONTAL)}
            outline={focus_outline(focused, theme.accent, FOCUS_RING_WIDTH, 1.0)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <Text size=TextSize::Sm color={glyph} align=TextAlign::Center>
                        <Icon glyph=ICON_TAB_GROUP />
                    </Text>
                </Show>
                <Show condition={pictured}>
                    <Text size=TextSize::Sm color={icon_color} align=TextAlign::Center>
                        <Icon glyph={icon} />
                    </Text>
                </Show>
                <Show condition={shows_picture}>
                    <TabPicture picture={picture} />
                </Show>
                <Text string={title} color={label} clip=true @sizing={title_size} />
                <Show condition={closable}>
                    {move || view! {
                        <IconButton
                            @test_id={close_test_id.clone()}
                            glyph=ICON_CLOSE
                            label="Close tab"
                            size=IconButtonSize::Compact
                            variant=ButtonVariant::Ghost
                            capture_presses=true
                            on_click={close}
                        />
                    }}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn TabPicture(picture: Memo<Option<Image>>) -> NodeId {
    view! {
        <Frame width=GROUP_GLYPH height=GROUP_GLYPH>
            <Picture image={picture} />
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
    let outline = create_memo(move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    let border = match barred {
        true => CHROME_BORDER,
        false => 0.0,
    };
    view! {
        <Frame
            fill={theme.background}
            outline={create_memo(move || barred.then(|| Outline::new(outline.get(), CHROME_BORDER)))}
            radius={if barred { CARD_RADIUS } else { 0.0 }}
            padding=border
        >
            {content}
        </Frame>
    }
}

#[component]
fn DockDialogFace(handle: DockDialogHandle) -> NodeId {
    let DockDialogHandle {
        tab,
        title,
        icon,
        size,
        focused,
        attention,
        closable,
        close,
        body,
        ..
    } = handle;
    let theme = use_theme();
    let outline = create_memo(move || match (attention.get(), focused.get()) {
        (true, _) => theme.danger.get(),
        (false, true) => theme.accent.get(),
        (false, false) => theme.border.get(),
    });
    let pictured = create_memo(move || !icon.get().is_empty());
    let width = create_memo(move || size.get().x + 2.0 * CHROME_BORDER);
    view! {
        <Frame
            width=width
            fill={theme.background}
            outline={create_memo(move || Some(Outline::new(outline.get(), CHROME_BORDER)))}
            radius=CARD_RADIUS
            padding=CHROME_BORDER
        >
            <List spacing=0.0>
                <Frame
                    fill={theme.surface}
                    height=TAB_HEIGHT
                    padding={Sides::horizontal(TAB_PADDING_HORIZONTAL)}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                        <Show condition={pictured}>
                            <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={icon} />
                            </Text>
                        </Show>
                        <Text string={title} clip=true @sizing=ItemSize::Percent(100.0) />
                        <Show condition={closable}>
                            {move || view! {
                                <IconButton
                                    @test_id={format!("dock.dialog.{}.close", tab.value())}
                                    glyph=ICON_CLOSE
                                    label="Close"
                                    size=IconButtonSize::Compact
                                    variant=ButtonVariant::Ghost
                                    capture_presses=true
                                    on_click={close}
                                />
                            }}
                        </Show>
                    </List>
                </Frame>
                {body}
            </List>
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
        vacant,
        closable,
        menu,
        close,
        ..
    } = handle;
    let close = move || close.call();
    match vertical {
        true => view! {
            <DockSideBar grip tabs title vacant menu closable close />
        },
        false => view! {
            <DockTitleBar grip tabs title vacant menu closable close />
        },
    }
}

#[component]
fn DockTitleBar(
    grip: NodeId,
    tabs: Option<NodeId>,
    title: Prop<String>,
    vacant: Memo<bool>,
    menu: Memo<Vec<Action>>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let rule = bar_rule(vacant);
    view! {
        <List spacing=0.0>
            <Frame fill=fill padding=BAR_PADDING>
                <Frame height={Extent::at_least(TAB_HEIGHT)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                        {grip}
                        {tabs} @sizing=ItemSize::Percent(100.0)
                        <Show condition={titled}>
                            <Text string={title.clone()} @sizing=ItemSize::Percent(100.0) />
                        </Show>
                        <DockMenu menu />
                        <DockClose closable close={close} />
                    </List>
                </Frame>
            </Frame>
            <Frame fill=rule height=CHROME_BORDER />
        </List>
    }
}

#[component]
fn DockSideBar(
    grip: NodeId,
    tabs: Option<NodeId>,
    title: Prop<String>,
    vacant: Memo<bool>,
    menu: Memo<Vec<Action>>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let rule = bar_rule(vacant);
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
        <Frame
            fill=fill
            padding=BAR_PADDING
            @sizing=ItemSize::Percent(100.0)
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
                        <DockClose closable close={close} />
                    </List>
                </List>
                {tabs} @sizing=ItemSize::Percent(100.0)
                <Show condition={titled}>
                    <Text string={title.clone()} />
                </Show>
            </List>
        </Frame>
        <Frame fill=rule width=CHROME_BORDER />
        </List>
    }
}

#[component]
pub(crate) fn DockMenu(
    menu: Memo<Vec<Action>>,
    #[prop(default = IconButtonSize::Compact)] size: IconButtonSize,
) -> DynamicSegment<ListChild> {
    let offered = create_memo(move || menu.with(|items| !items.is_empty()));
    let keys =
        create_memo(move || menu.with(|items| items.iter().map(Action::key).collect::<Vec<u64>>()));
    view! {
        <Show condition={offered}>
            {move || view! {
                <IconMenuButton
                    @test_id={"dock.menu"}
                    label="More"
                    glyph=ICON_MORE_VERT
                    size
                    items={view! {
                        <ForEach keys={keys}>
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
            }}
        </Show>
    }
}

#[component]
fn DockClose(closable: Memo<bool>, close: ClickCallback) -> NodeId {
    view! {
        <Frame padding={Sides::horizontal(CONTROL_PADDING)} visible={closable}>
            <IconButton
                glyph=ICON_CLOSE
                label="Close pane"
                size=IconButtonSize::Compact
                variant=ButtonVariant::Ghost
                capture_presses=true
                on_click={close}
            />
        </Frame>
    }
}

#[component]
fn DockGrip(handle: DockGripHandle) -> NodeId {
    let DockGripHandle { focused, .. } = handle;
    let color = create_memo(move || match focused.get() {
        true => TextColor::Muted,
        false => TextColor::Faint,
    });
    view! {
        <Frame width=GRIP_WIDTH padding={Sides::symmetric(GRIP_PADDING, CONTROL_PADDING)}>
            <Text size=TextSize::Sm color={color} align=TextAlign::Center>
                <Icon glyph=ICON_DRAG_INDICATOR />
            </Text>
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
    let fill = create_memo(move || match (active.get(), hovered.get()) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.border.get(),
        (false, false) => theme.background.get(),
    });
    view! {
        <Frame fill=fill outline={focus_outline(focused, theme.accent, FOCUS_RING_WIDTH, 0.0)} />
    }
}

#[component]
fn DockDropHighlight() -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || translucent(theme.accent.get(), DROP_ALPHA));
    view! {
        <Frame
            @test_id={"dock.drop"}
            fill=fill
            outline={create_memo(move || Some(Outline::new(theme.accent.get(), FOCUS_RING_WIDTH)))}
            radius=RADIUS
        />
    }
}

#[component]
fn DockDragPreview(title: Prop<String>, icon: Memo<String>, grouped: bool) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || translucent(theme.surface_raised.get(), PREVIEW_ALPHA));
    let pictured = create_memo(move || !grouped && !icon.get().is_empty());
    let pictured_color = TextColor::Default;
    view! {
        <Frame
            fill=fill
            outline={create_memo(move || Some(Outline::new(theme.accent.get(), FOCUS_RING_WIDTH)))}
            radius=RADIUS
            height=TAB_HEIGHT
            padding={Sides::horizontal(PREVIEW_PADDING)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <Text size=TextSize::Sm align=TextAlign::Center>
                        <Icon glyph=ICON_TAB_GROUP />
                    </Text>
                </Show>
                <Show condition={pictured}>
                    <Text size=TextSize::Sm color={pictured_color} align=TextAlign::Center>
                        <Icon glyph={icon} />
                    </Text>
                </Show>
                <Text string={title} />
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

fn bar_fill(vacant: Memo<bool>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || match vacant.get() {
        true => theme.background.get(),
        false => theme.surface.get(),
    })
}

fn bar_rule(vacant: Memo<bool>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || match vacant.get() {
        true => theme.background.get(),
        false => theme.border.get(),
    })
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}
