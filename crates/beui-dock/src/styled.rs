use beui_core::font::Overflow;
use beui_core::icons::Glyph;
mod stack;
mod switch;

use beui_components_styled::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_components_styled::sheet::ModalSheet;
use beui_macros::{component, view};

use crate::unstyled as dock;
use crate::unstyled::{
    DockAwayHandle, DockBarHandle, DockChromeHandle, DockDialogHandle, DockDialogPresentation,
    DockDragged, DockGripHandle, DockKey, DockMode, DockNode, DockPreviewHandle, DockSheetHandle,
    DockSplitterHandle, DockStackHandle, DockSwitchHandle, DockSwitcherHandle, DockTabHandle,
    DockingLayout, Entry,
};
use beui_components_styled::button::{Button, ButtonSize, ButtonVariant};
use beui_components_styled::context_menu::menu_style;
use beui_components_styled::menu_button::MenuButton;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize};
use beui_components_styled::theme::{NARROW_WIDTH, POINTER_REACH, use_theme};
use beui_components_styled::tooltip::Tooltip;
use beui_components_unstyled::{MenuNodes, Picture};
use beui_core::base::frame::Align2;
use beui_core::base::{Align, Direction, ItemSize, Justify};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::icons::{
    ICON_APP_WINDOW, ICON_ELLIPSIS_VERTICAL, ICON_GRIP_VERTICAL, ICON_MAXIMIZE, ICON_MINIMIZE,
    ICON_X,
};
use beui_core::image::Image;
use beui_core::input::Modifiers;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;
use stack::{DockStackBar, DockSwitcher};
use switch::DockSwitchPanel;

const TAB_PADDING_HORIZONTAL: f32 = 10.0;
const TAB_HEIGHT: f32 = 30.0;
const TAB_RADIUS: Radius = Radius::custom(7.0);
const TAB_SPACING: f32 = 8.0;
const BAR_HEIGHT: f32 = 39.0;
const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 2.0;
const DIVIDER_HEIGHT: f32 = 18.0;
const DIVIDER_MARGIN: f32 = 4.0;
const SHARE_MARGIN: f32 = 2.0;
const GRIP_WIDTH: f32 = 16.0;
const ICON_SIDEBAR: f32 = TAB_HEIGHT + 2.0 * BAR_PADDING;
const PREVIEW_PADDING: f32 = 8.0;
pub const CHROME_BORDER: f32 = 2.0;
const RULE: f32 = 1.0;
const SPLITTER_LINE: f32 = 1.0;
const SPLITTER_REACH: f32 = 3.0;
const SPLITTER_HELD: f32 = 3.0;
const GROUP_GLYPH: f32 = 16.0;
pub const WINDOW_CHROME: beui_core::geometry::Vec2 =
    beui_core::geometry::Vec2::new(2.0 * CHROME_BORDER, BAR_HEIGHT + 2.0 * CHROME_BORDER + RULE);
const DROP_ALPHA: u8 = 64;
const AWAY_SPACING: f32 = 12.0;
const AWAY_PADDING: f32 = 24.0;
const PREVIEW_ALPHA: u8 = 235;
const WAITING_DOT: f32 = 6.0;
const CARD_POINTER_ROOM: Sides = Sides::all(POINTER_REACH);
const CARD_MARGIN: u16 = (beui_components_styled::dialog::MARGIN - CARD_POINTER_ROOM.top) as u16;
const CARD_GAP: u16 = (POINTER_REACH + 2.0) as u16;

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    #[prop(default = NARROW_WIDTH)] stack_below: f32,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let mode = layout.mode();
    let theme = use_theme();
    let backdrop = theme.background;
    let dialog_scrim = create_memo(move || theme.scrim.get());
    let fill = create_memo(move || match mode.get() {
        DockMode::Tiled => Color32::TRANSPARENT,
        DockMode::Stacked => theme.background.get(),
    });
    view! {
        <dock::Docking
            layout
            stack_below
            focus
            drag_modifier
            splitter_thickness=SPLITTER_LINE
            splitter_reach=SPLITTER_REACH
            icon_sidebar_width=ICON_SIDEBAR
            menu={menu_style()}
            children
            frame={move |body: NodeId| view! {
                <Frame fill={ColorRole::custom(fill)}>{body}</Frame>
            }}
            fullscreen={move |body: NodeId| view! {
                <Frame fill={ColorRole::custom(backdrop)}>{body}</Frame>
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
            dialog_sheet={move |handle: DockSheetHandle| view! {
                <DockDialogSheet handle />
            }}
            dialog_scrim={dialog_scrim}
            dialog_margin=CARD_MARGIN
            dialog_gap=CARD_GAP
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
    let message = create_memo(move || format!("{} is full screen", title.get()));
    view! {
        <Frame
            fill=ColorRole::Background
            align=Align::Center
            padding=AWAY_PADDING
            @test_id="dock.away"
        >
            <List align=Align::Center spacing=AWAY_SPACING>
                <Text size=TextSize::Xl3 color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_MAXIMIZE />
                </Text>
                <Text align=TextAlign::Center>{message}</Text>
                <Button
                    label="Leave full screen"
                    glyph=ICON_MINIMIZE
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
        waiting,
        close,
        vertical,
        icons_only,
        ..
    } = handle;
    let grouped = matches!(entry, Entry::Group(_));
    view! {
        <List spacing=0.0>
            <Dynamic value={icons_only}>|icons_only: bool| {
                match icons_only {
                    true => view! {
                        <DockIconTab
                            title
                            icon
                            picture
                            grouped
                            selected
                            hovered
                            active
                            focused
                            dragged
                            waiting
                        />
                    },
                    false => view! {
                        <DockTabChrome
                            title
                            icon
                            picture
                            grouped
                            vertical
                            selected
                            hovered
                            active
                            focused
                            dragged
                            closable
                            waiting
                            waiting_test_id={waiting_test_id(entry)}
                            close_test_id={close_test_id(entry)}
                            close={close}
                        />
                    },
                }
            }</Dynamic>
        </List>
    }
}

fn tab_fill(
    selected: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    dragged: Memo<bool>,
) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(
        move || match (selected.get(), hovered.get() || active.get(), dragged.get()) {
            (_, _, true) => theme.accent_soft.get(),
            (true, _, false) => theme.surface_raised.get(),
            (false, true, false) => theme.hover.get(),
            (false, false, false) => theme.surface.get(),
        },
    )
}

#[component]
fn DockIconTab(
    title: Memo<String>,
    icon: Memo<Option<Glyph>>,
    picture: Memo<Option<Image>>,
    grouped: bool,
    selected: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    dragged: Memo<bool>,
    waiting: Memo<bool>,
) -> NodeId {
    let fill = tab_fill(selected, hovered, active, dragged);
    let label = create_memo(move || match (waiting.get(), selected.get()) {
        (true, _) => TextColor::Accent,
        (false, true) => TextColor::Default,
        (false, false) => TextColor::Muted,
    });
    let shows_picture = create_memo(move || !grouped && picture.with(Option::is_some));
    let glyph = create_memo(move || match grouped {
        true => Some(ICON_APP_WINDOW),
        false => icon.get(),
    });
    let pictured = create_memo(move || !shows_picture.get() && glyph.get().is_some());
    let lettered = create_memo(move || !shows_picture.get() && glyph.get().is_none());
    let initial = create_memo(move || {
        title
            .with(|title| {
                title
                    .chars()
                    .next()
                    .map(char::to_uppercase)
                    .map(String::from_iter)
            })
            .unwrap_or_default()
    });
    view! {
        <Tooltip label={title}>
            <Frame
                fill={ColorRole::custom(fill)}
                radius=TAB_RADIUS
                width=TAB_HEIGHT
                height=TAB_HEIGHT
                align=Align::Center
                border={focus_border(focused, Border::FOCUS)}
            >
                <List align=Align::Center spacing=0.0>
                    <Show condition={pictured}>
                        <Text color={label} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Show>
                    <Show condition={shows_picture}>
                        <TabPicture picture={picture} />
                    </Show>
                    <Show condition={lettered}>
                        <Text color={label} align=TextAlign::Center>{initial}</Text>
                    </Show>
                </List>
            </Frame>
        </Tooltip>
    }
}

#[component]
fn DockTabChrome(
    title: Prop<String>,
    icon: Memo<Option<Glyph>>,
    picture: Memo<Option<Image>>,
    grouped: bool,
    vertical: bool,
    selected: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    dragged: Memo<bool>,
    closable: Memo<bool>,
    waiting: Memo<bool>,
    waiting_test_id: String,
    close_test_id: String,
    close: ClickCallback,
) -> NodeId {
    let fill = tab_fill(selected, hovered, active, dragged);
    let label = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let shows_picture = create_memo(move || !grouped && picture.with(Option::is_some));
    let pictured = create_memo(move || !grouped && !shows_picture.get() && icon.get().is_some());
    let title_size = match vertical {
        true => ItemSize::Percent(100.0),
        false => ItemSize::Intrinsic,
    };
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=TAB_RADIUS
            height=TAB_HEIGHT
            padding={Sides::horizontal(TAB_PADDING_HORIZONTAL)}
            border={focus_border(focused, Border::FOCUS)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <Text color={label} align=TextAlign::Center>
                        <Icon glyph=ICON_APP_WINDOW />
                    </Text>
                </Show>
                <Show condition={pictured}>
                    <Text color={label} align=TextAlign::Center>
                        <Icon glyph={icon} />
                    </Text>
                </Show>
                <Show condition={shows_picture}>
                    <TabPicture picture={picture} />
                </Show>
                <Text color={label} overflow=Overflow::Ellipsis @sizing={title_size}>{title}</Text>
                <Show condition={waiting}>
                    <Frame
                        @test_id={waiting_test_id.clone()}
                        width=WAITING_DOT
                        height=WAITING_DOT
                        radius=Radius::Full
                        fill=ColorRole::Accent
                    />
                </Show>
                <Show condition={closable}>
                    <Button
                        icon_only=true
                        @test_id={close_test_id.clone()}
                        glyph=ICON_X
                        label="Close tab"
                        size=ButtonSize::Small
                        variant=ButtonVariant::Ghost
                        capture_presses=true
                        on_click={close}
                    />
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
        floating,
        barred,
        focused,
        content,
    } = handle;
    let framed = barred && floating;
    let outline = create_memo(move || {
        framed.then(|| {
            let color = match focused.get() {
                true => ColorRole::Accent,
                false => ColorRole::Border,
            };
            Border::new(color).width(CHROME_BORDER)
        })
    });
    let border = match framed {
        true => CHROME_BORDER,
        false => 0.0,
    };
    view! {
        <Frame
            fill=ColorRole::Background
            border=outline
            radius={if framed { Radius::Lg } else { Radius::None }}
            padding=border
        >
            {content}
        </Frame>
    }
}

#[component]
fn DockDialogFace(handle: DockDialogHandle) -> NodeId {
    match handle.presentation {
        DockDialogPresentation::Window => view! {
            <DockWindowFace handle />
        },
        DockDialogPresentation::Card => view! {
            <DockCardFace handle />
        },
    }
}

#[component]
fn DockDialogSheet(handle: DockSheetHandle) -> NodeId {
    let DockSheetHandle {
        open, close, body, ..
    } = handle;
    view! {
        <ModalSheet open fit=true on_close={close}>{body}</ModalSheet>
    }
}

fn dialog_outline_color(focused: Memo<bool>, attention: ReadSignal<bool>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || match (attention.get(), focused.get()) {
        (true, _) => theme.danger.get(),
        (false, true) => theme.accent.get(),
        (false, false) => theme.border.get(),
    })
}

fn dialog_outline(focused: Memo<bool>, attention: ReadSignal<bool>) -> Memo<Option<Border>> {
    let color = dialog_outline_color(focused, attention);
    create_memo(move || Some(Border::new(ColorRole::Custom(color.get())).width(CHROME_BORDER)))
}

#[component]
fn DockCardFace(handle: DockDialogHandle) -> NodeId {
    let DockDialogHandle {
        focused,
        attention,
        pointer,
        body,
        ..
    } = handle;
    view! {
        <Frame padding=CARD_POINTER_ROOM>
            <Frame
                @node_ref=&pointer
                fill=ColorRole::SurfaceRaised
                border={dialog_outline(focused, attention)}
                radius=Radius::Lg
                pointer=true
                padding=CHROME_BORDER
            >
                {body}
            </Frame>
        </Frame>
    }
}

#[component]
fn DockWindowFace(handle: DockDialogHandle) -> NodeId {
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
    let pictured = create_memo(move || icon.get().is_some());
    let width = create_memo(move || size.get().x + 2.0 * CHROME_BORDER);
    view! {
        <Frame
            width=width
            fill=ColorRole::Background
            border={dialog_outline(focused, attention)}
            radius=Radius::Lg
            padding=CHROME_BORDER
        >
            <List spacing=0.0>
                <Frame
                    fill=ColorRole::Surface
                    height=TAB_HEIGHT
                    padding={Sides::horizontal(TAB_PADDING_HORIZONTAL)}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                        <Show condition={pictured}>
                            <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={icon} />
                            </Text>
                        </Show>
                        <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>
                            {title}
                        </Text>
                        <Show condition={closable}>
                            <Button
                                icon_only=true
                                @test_id={format!("dock.dialog.{}.close", tab.value())}
                                glyph=ICON_X
                                label="Close"
                                size=ButtonSize::Small
                                variant=ButtonVariant::Ghost
                                capture_presses=true
                                on_click={close}
                            />
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
        icons_only,
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
            <DockSideBar grip tabs title vacant menu closable icons_only close />
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
    menu: Memo<Vec<MenuNode>>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let rule = bar_rule(vacant);
    view! {
        <List spacing=0.0>
            <Frame
                fill={ColorRole::custom(fill)}
                height=BAR_HEIGHT
                padding={Sides::horizontal(BAR_PADDING)}
                align={Align2::new(Align::Stretch, Align::Center)}
            >
                <Frame>
                    <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                        {grip}
                        {tabs} @sizing=ItemSize::Percent(100.0)
                        <Show condition={titled}>|| {
                            let title = title.clone(); view! {
                                <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>
                                    {title}
                                </Text>
                            }
                        }</Show>
                        <DockMenu menu room=BarRoom::Wide />
                        <DockClose closable close={close} />
                    </List>
                </Frame>
            </Frame>
            <Frame fill={ColorRole::custom(rule)} height=RULE />
        </List>
    }
}

#[component]
fn DockSideBar(
    grip: NodeId,
    tabs: Option<NodeId>,
    title: Prop<String>,
    vacant: Memo<bool>,
    menu: Memo<Vec<MenuNode>>,
    closable: Memo<bool>,
    icons_only: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let titles = create_memo(move || !icons_only.get());
    let header = create_memo(move || match icons_only.get() {
        true => Justify::Center,
        false => Justify::SpaceBetween,
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                fill={ColorRole::custom(fill)}
                padding=BAR_PADDING
                @sizing=ItemSize::Percent(100.0)
            >
                <List spacing=BAR_SPACING>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        justify={header}
                        spacing=BAR_SPACING
                    >
                        {grip}
                        <Show condition={titles}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=BAR_SPACING
                            >
                                <DockMenu menu room=BarRoom::Narrow />
                                <DockClose closable close={close} />
                            </List>
                        </Show>
                    </List>
                    {tabs} @sizing=ItemSize::Percent(100.0)
                    <Show condition={titled}>|| {
                        let title = title.clone(); view! {
                            <Text overflow=Overflow::Ellipsis>{title}</Text>
                        }
                    }</Show>
                    <Show condition={icons_only}>
                        <List align=Align::Center spacing=BAR_SPACING>
                            <DockMenu menu room=BarRoom::Column />
                            <DockClose closable close={close} />
                        </List>
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarRoom {
    Wide,
    Narrow,
    Column,
    Phone,
}

impl BarRoom {
    fn button(self) -> ButtonSize {
        match self {
            BarRoom::Wide | BarRoom::Narrow | BarRoom::Column => ButtonSize::Toolbar,
            BarRoom::Phone => ButtonSize::Regular,
        }
    }

    fn direction(self) -> Direction {
        match self {
            BarRoom::Column => Direction::Vertical,
            BarRoom::Wide | BarRoom::Narrow | BarRoom::Phone => Direction::Horizontal,
        }
    }

    fn shows(self, pin: Pin) -> bool {
        match pin {
            Pin::Icon => true,
            Pin::Label => self != BarRoom::Phone,
            Pin::Title => self == BarRoom::Phone,
        }
    }
}

#[component]
pub(crate) fn DockMenu(menu: Memo<Vec<MenuNode>>, room: BarRoom) -> NodeId {
    let pins = create_memo(move || {
        menu.with(|nodes| {
            pinned(nodes)
                .into_iter()
                .filter(|action| {
                    action
                        .pinned()
                        .is_some_and(|pin| pin != Pin::Title && room.shows(pin))
                })
                .collect::<Vec<_>>()
        })
    });
    let keys_of = move |labelled: bool| {
        create_memo(move || {
            pins.with(|pins| {
                pins.iter()
                    .filter(|pin| (pin.pinned() == Some(Pin::Label)) == labelled)
                    .map(Action::key)
                    .collect::<Vec<_>>()
            })
        })
    };
    let icons = keys_of(false);
    let labels = keys_of(true);
    let divided = create_memo(move || {
        room == BarRoom::Wide
            && icons.with(|icons| !icons.is_empty())
            && labels.with(|labels| !labels.is_empty())
    });
    let rest = create_memo(move || unpinned(menu.get(), room));
    let offered = create_memo(move || rest.with(|items| !items.is_empty()));
    let pin = move |key: u64| {
        let action = pins.with_untracked(|pins| pins.iter().find(|pin| pin.key() == key).cloned());
        match action {
            Some(action) => view! {
                <DockPin action room />
            },
            None => view! {
                <Frame />
            },
        }
    };
    view! {
        <List direction={room.direction()} align=Align::Center spacing=BAR_SPACING>
            <ForEach keys={icons}>|key: u64| { pin(key) }</ForEach>
            <Show condition={divided}>
                <Frame padding={Sides::horizontal(DIVIDER_MARGIN)}>
                    <Frame width=RULE height=DIVIDER_HEIGHT fill=ColorRole::Border />
                </Frame>
            </Show>
            <ForEach keys={labels}>|key: u64| { pin(key) }</ForEach>
            <Show condition={offered}>
                <MenuButton
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    @test_id={"dock.menu"}
                    label="More"
                    glyph=ICON_ELLIPSIS_VERTICAL
                    size={room.button()}
                    items={view! {
                        <MenuNodes nodes={rest} test_prefix={"dock.menu".to_owned()} />
                    }}
                />
            </Show>
        </List>
    }
}

#[component]
fn DockPin(action: Action, room: BarRoom) -> NodeId {
    let id = format!("dock.pin.{}", action.id());
    match room == BarRoom::Wide && action.pinned() == Some(Pin::Label) {
        true => view! {
            <Frame padding={Sides::horizontal(SHARE_MARGIN)}>
                <Button
                    @test_id={id}
                    action={action}
                    variant=ButtonVariant::Primary
                    size=ButtonSize::Compact
                />
            </Frame>
        },
        false => view! {
            <Button
                icon_only=true
                @test_id={id}
                action={action}
                variant=ButtonVariant::Ghost
                size={room.button()}
            />
        },
    }
}

pub(crate) fn title_action(nodes: &[MenuNode]) -> Option<Action> {
    pinned(nodes)
        .into_iter()
        .find(|action| action.pinned() == Some(Pin::Title))
}

fn pinned(nodes: &[MenuNode]) -> Vec<Action> {
    nodes
        .iter()
        .flat_map(|node| match node {
            MenuNode::Action { action, .. } if action.pinned().is_some() => vec![action.clone()],
            MenuNode::Submenu(submenu) => pinned(&submenu.items),
            MenuNode::Action { .. } | MenuNode::Separator => Vec::new(),
        })
        .collect()
}

fn unpinned(nodes: Vec<MenuNode>, room: BarRoom) -> Vec<MenuNode> {
    let kept = nodes
        .into_iter()
        .filter_map(|node| match node {
            MenuNode::Action { action, .. }
                if action.pinned().is_some_and(|pin| room.shows(pin)) =>
            {
                None
            }
            MenuNode::Submenu(mut submenu) => {
                submenu.items = unpinned(std::mem::take(&mut submenu.items), room);
                Some(MenuNode::Submenu(submenu))
            }
            node => Some(node),
        })
        .collect();
    tidy_menu(kept)
}

#[component]
fn DockClose(closable: Memo<bool>, close: ClickCallback) -> DynamicSegment<ListChild> {
    view! {
        <Show condition={closable}>
            <Button
                icon_only=true
                glyph=ICON_X
                label="Close pane"
                size=ButtonSize::Toolbar
                variant=ButtonVariant::Ghost
                capture_presses=true
                on_click={close}
            />
        </Show>
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
        <Frame width=GRIP_WIDTH height=TAB_HEIGHT align=Align::Center>
            <Text size=TextSize::Sm color={color} align=TextAlign::Center>
                <Icon glyph=ICON_GRIP_VERTICAL />
            </Text>
        </Frame>
    }
}

#[component]
fn DockSplitterFace(handle: DockSplitterHandle) -> NodeId {
    let DockSplitterHandle {
        direction,
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let lit = create_memo(move || active.get() || hovered.get());
    let line = create_memo(move || match (active.get(), hovered.get()) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.text_muted.get(),
        (false, false) => theme.border.get(),
    });
    let padding = create_memo(move || {
        let side = match lit.get() {
            true => (SPLITTER_REACH - SPLITTER_HELD / 2.0 + SPLITTER_LINE / 2.0).max(0.0),
            false => SPLITTER_REACH,
        };
        match direction {
            Direction::Horizontal => Sides::horizontal(side),
            Direction::Vertical => Sides::vertical(side),
        }
    });
    view! {
        <Frame padding={padding} border={focus_border(focused, Border::FOCUS)}>
            <Frame fill={ColorRole::custom(line)} />
        </Frame>
    }
}

#[component]
fn DockDropHighlight() -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || translucent(theme.accent.get(), DROP_ALPHA));
    view! {
        <Frame
            @test_id={"dock.drop"}
            fill={ColorRole::custom(fill)}
            border={Border::FOCUS}
            radius=Radius::Md
        />
    }
}

#[component]
fn DockDragPreview(title: Prop<String>, icon: Memo<Option<Glyph>>, grouped: bool) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || translucent(theme.surface_raised.get(), PREVIEW_ALPHA));
    let pictured = create_memo(move || !grouped && icon.get().is_some());
    let pictured_color = TextColor::Default;
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            border={Border::FOCUS}
            radius=TAB_RADIUS
            height=TAB_HEIGHT
            padding={Sides::horizontal(PREVIEW_PADDING)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <Text size=TextSize::Sm align=TextAlign::Center>
                        <Icon glyph=ICON_APP_WINDOW />
                    </Text>
                </Show>
                <Show condition={pictured}>
                    <Text size=TextSize::Sm color={pictured_color} align=TextAlign::Center>
                        <Icon glyph={icon} />
                    </Text>
                </Show>
                <Text overflow=Overflow::Ellipsis>{title}</Text>
            </List>
        </Frame>
    }
}

fn waiting_test_id(entry: Entry) -> String {
    match entry {
        Entry::Tab(tab) => format!("dock.tab.{}.waiting", tab.value()),
        Entry::Group(_) => String::new(),
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
