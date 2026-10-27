use beui_view::types::Overflow;
use beui_view::types::icons::Glyph;
mod stack;
mod switch;

use beui_components_styled::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_components_styled::sheet::ModalSheet;
use beui_macros::{component, view};

use crate::unstyled as dock;
pub use crate::unstyled::{
    DockApi, DockDialog, DockDialogPresentation, DockDocument, DockDocumentBody, DockDocumentTabs,
    DockDocuments, DockEntry, DockMode, DockNode, DockPane, DockSlot, DockSplit, DockTab,
    DockWindow, DockingLayout, DockingSnapshot, TabId, dock_menu, use_dock_tab,
};
use crate::unstyled::{
    DockAwayHandle, DockBarHandle, DockChromeHandle, DockDialogHandle, DockDragged, DockGripHandle,
    DockKey, DockPreviewHandle, DockSheetHandle, DockSplitterHandle, DockStackHandle,
    DockSwitchHandle, DockSwitcherHandle, DockTabHandle, Entry,
};
use beui_components_styled::List;
use beui_components_styled::action_bar::{ActionBar, ActionLabels};
use beui_components_styled::button::{Button, ButtonSize, ButtonVariant};
use beui_components_styled::context_menu::menu_style;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize};
use beui_components_styled::theme::{NARROW_WIDTH, POINTER_REACH, use_theme};
use beui_components_styled::tokens::{Padding, Size, Space, Stroke};
use beui_components_styled::tooltip::Tooltip;
use beui_components_unstyled::Picture;
use beui_view::types::Align2;
use beui_view::types::{Align, Direction, ItemSize, Justify};

use beui_view::primitives::Layers;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Modifiers;
use beui_view::types::Sides;
use beui_view::types::TextAlign;
use beui_view::types::icons::{
    ICON_APP_WINDOW, ICON_GRIP_VERTICAL, ICON_MAXIMIZE, ICON_MINIMIZE, ICON_X,
};
use stack::{DockStackBar, DockSwitcher};
use switch::DockSwitchPanel;

const TAB_PADDING_HORIZONTAL: Space = Space::Md;
const TAB_HEIGHT: Size = Size::Toolbar;
const TAB_RADIUS: Radius = Radius::Tab;
const TAB_SPACING: Space = Space::Md;
const BAR_HEIGHT: f32 = 39.0;
const BAR_PADDING: Space = Space::Xs;
const BAR_SPACING: Space = Space::Xs2;
const GRIP_WIDTH: Size = Size::Xs;
const ICON_SIDEBAR: f32 = TAB_HEIGHT.px() + 2.0 * BAR_PADDING.px();
const PREVIEW_PADDING: Space = Space::Md;
pub const CHROME_BORDER: f32 = CHROME_STROKE.px();
const CHROME_STROKE: Stroke = Stroke::Focus;
const CHROME_PADDING: Space = Space::Xs2;
const RULE: f32 = 1.0;
const SPLITTER_LINE: f32 = 1.0;
const TAB_STRIP_SPACING: Space = Space::Sm;
const DROP_MARKER: Stroke = Stroke::Focus;
const SPLITTER_REACH: f32 = 3.0;
const SPLITTER_HELD: f32 = 3.0;
const GROUP_GLYPH: Size = Size::Xs;
pub const WINDOW_CHROME: beui_view::types::Vec2 =
    beui_view::types::Vec2::new(2.0 * CHROME_BORDER, BAR_HEIGHT + 2.0 * CHROME_BORDER + RULE);
const DIALOG_CHROME: beui_view::types::Vec2 =
    beui_view::types::Vec2::new(2.0 * CHROME_BORDER, TAB_HEIGHT.px() + 2.0 * CHROME_BORDER);
const DROP_OPACITY: f32 = 0.25;
const AWAY_SPACING: Space = Space::Lg;
const AWAY_PADDING: Space = Space::Xl3;
const PREVIEW_OPACITY: f32 = 0.92;
const WAITING_DOT: Size = Size::Custom(6.0);
const CARD_POINTER_ROOM: Sides = Sides::all(POINTER_REACH);
const CARD_MARGIN: u16 =
    (beui_components_styled::dialog::MARGIN.px() - CARD_POINTER_ROOM.top) as u16;
const CARD_GAP: u16 = (POINTER_REACH + 2.0) as u16;

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let mode = layout.mode();
    let theme = use_theme();
    let dialog_scrim = create_memo(move || ColorRole::Scrim.resolve(&theme));
    let back_shade = create_memo(move || ColorRole::ScrimSoft.resolve(&theme));
    let fill = create_memo(move || match mode.get() {
        DockMode::Tiled => ColorRole::None,
        DockMode::Stacked => ColorRole::Background,
    });
    view! {
        <dock::Docking
            layout
            stack_below=NARROW_WIDTH
            focus
            drag_modifier
            splitter_thickness=SPLITTER_LINE
            splitter_reach=SPLITTER_REACH
            icon_sidebar_width=ICON_SIDEBAR
            tab_spacing={TAB_STRIP_SPACING.px()}
            drop_marker_width={DROP_MARKER.px()}
            back_shade={back_shade}
            menu={menu_style()}
            children
            frame={move |body: NodeId| view! {
                <Frame fill={fill}>{body}</Frame>
            }}
            fullscreen={move |body: NodeId| view! {
                <Frame fill=ColorRole::Background>{body}</Frame>
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
            dialog_beak=POINTER_REACH
            dialog_outset=DIALOG_CHROME
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
        notice,
        close,
        vertical,
        icons_only,
        ..
    } = handle;
    let grouped = matches!(entry, Entry::Group(_));
    let noticed = create_memo(move || notice.with(Option::is_some));
    let title = create_memo(move || notice.get().unwrap_or_else(|| title.get()));
    view! {
        <List spacing=Space::None>
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
                            noticed
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
                            noticed
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
    noticed: Memo<bool>,
) -> Memo<ColorRole> {
    create_memo(
        move || match (selected.get(), hovered.get() || active.get(), dragged.get()) {
            (_, _, true) => ColorRole::AccentSoft,
            _ if noticed.get() => ColorRole::AccentSoft,
            (true, _, false) => ColorRole::SurfaceRaised,
            (false, true, false) => ColorRole::Hover,
            (false, false, false) => ColorRole::Surface,
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
    noticed: Memo<bool>,
) -> NodeId {
    let fill = tab_fill(selected, hovered, active, dragged, noticed);
    let label = create_memo(
        move || match (waiting.get() || noticed.get(), selected.get()) {
            (true, _) => TextColor::Accent,
            (false, true) => TextColor::Default,
            (false, false) => TextColor::Muted,
        },
    );
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
                fill={fill}
                radius=TAB_RADIUS
                width=TAB_HEIGHT
                height=TAB_HEIGHT
                align=Align::Center
                border={focus_border(focused, Border::FOCUS)}
            >
                <List align=Align::Center spacing=Space::None>
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
    noticed: Memo<bool>,
    waiting_test_id: String,
    close_test_id: String,
    close: ClickCallback,
) -> NodeId {
    let fill = tab_fill(selected, hovered, active, dragged, noticed);
    let label = create_memo(move || match (noticed.get(), selected.get()) {
        (true, _) => TextColor::Accent,
        (false, true) => TextColor::Default,
        (false, false) => TextColor::Muted,
    });
    let shows_picture = create_memo(move || !grouped && picture.with(Option::is_some));
    let pictured = create_memo(move || !grouped && !shows_picture.get() && icon.get().is_some());
    let title_size = match vertical {
        true => ItemSize::Percent(100.0),
        false => ItemSize::Intrinsic,
    };
    view! {
        <Frame
            fill={fill}
            radius=TAB_RADIUS
            height=TAB_HEIGHT
            padding={Padding::horizontal(TAB_PADDING_HORIZONTAL)}
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
        see_through,
        content,
    } = handle;
    let framed = barred && floating;
    let fill = create_memo(move || match see_through.get() {
        true => ColorRole::None,
        false => ColorRole::Background,
    });
    let outline = create_memo(move || {
        framed.then(|| {
            let color = match focused.get() {
                true => ColorRole::Accent,
                false => ColorRole::Border,
            };
            Border::new(color).width(CHROME_STROKE)
        })
    });
    let border = match framed {
        true => CHROME_PADDING,
        false => Space::None,
    };
    view! {
        <Frame
            fill={fill}
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

fn dialog_outline(focused: Memo<bool>, attention: ReadSignal<bool>) -> Memo<Option<Border>> {
    create_memo(move || {
        let color = match (attention.get(), focused.get()) {
            (true, _) => ColorRole::Danger,
            (false, true) => ColorRole::Accent,
            (false, false) => ColorRole::Border,
        };
        Some(Border::new(color).width(CHROME_STROKE))
    })
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
        <Frame padding={Padding::custom(CARD_POINTER_ROOM)}>
            <Frame
                @node_ref=&pointer
                fill=ColorRole::SurfaceRaised
                border={dialog_outline(focused, attention)}
                radius=Radius::Lg
                padding=CHROME_PADDING
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
        notice,
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
    let width = create_memo(move || Size::Custom(size.get().x + 2.0 * CHROME_BORDER));
    let header = create_memo(move || match notice.with(Option::is_some) {
        true => ColorRole::AccentSoft,
        false => ColorRole::Surface,
    });
    let label = create_memo(move || match notice.with(Option::is_some) {
        true => TextColor::Accent,
        false => TextColor::Default,
    });
    let title = create_memo(move || notice.get().unwrap_or_else(|| title.get()));
    view! {
        <Frame
            width=width
            fill=ColorRole::Background
            border={dialog_outline(focused, attention)}
            radius=Radius::Lg
            padding=CHROME_PADDING
        >
            <List spacing=Space::None>
                <Frame
                    fill={header}
                    height=TAB_HEIGHT
                    padding={Padding::horizontal(TAB_PADDING_HORIZONTAL)}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                        <Show condition={pictured}>
                            <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={icon} />
                            </Text>
                        </Show>
                        <Text
                            color={label}
                            overflow=Overflow::Ellipsis
                            @sizing=ItemSize::Percent(100.0)
                        >
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
        <List spacing=Space::None>
            <Frame
                fill={fill}
                height={Size::Custom(BAR_HEIGHT)}
                padding={Padding::horizontal(BAR_PADDING)}
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
            <Frame fill={rule} height={Size::Custom(RULE)} />
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
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame fill={fill} padding=BAR_PADDING @sizing=ItemSize::Percent(100.0)>
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

#[component]
pub(crate) fn DockMenu(menu: Memo<Vec<MenuNode>>, room: BarRoom) -> NodeId {
    let labels = match room {
        BarRoom::Wide => ActionLabels::Shown,
        BarRoom::Narrow | BarRoom::Column => ActionLabels::Icons,
        BarRoom::Phone => ActionLabels::Overflow,
    };
    let direction = match room {
        BarRoom::Column => Direction::Vertical,
        BarRoom::Wide | BarRoom::Narrow | BarRoom::Phone => Direction::Horizontal,
    };
    let size = match room {
        BarRoom::Wide | BarRoom::Narrow | BarRoom::Column => ButtonSize::Toolbar,
        BarRoom::Phone => ButtonSize::Regular,
    };
    view! {
        <ActionBar
            menu
            labels
            title={room == BarRoom::Phone}
            direction
            size
            test_prefix={"dock".to_owned()}
        />
    }
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
    let lit = create_memo(move || active.get() || hovered.get());
    let line = create_memo(move || match (active.get(), hovered.get()) {
        (true, _) => ColorRole::Accent,
        (false, true) => ColorRole::TextMuted,
        (false, false) => ColorRole::Border,
    });
    let padding = create_memo(move || {
        let side = match lit.get() {
            true => (SPLITTER_REACH - SPLITTER_HELD / 2.0 + SPLITTER_LINE / 2.0).max(0.0),
            false => SPLITTER_REACH,
        };
        match direction {
            Direction::Horizontal => Padding::horizontal(Space::Custom(side)),
            Direction::Vertical => Padding::vertical(Space::Custom(side)),
        }
    });
    view! {
        <Frame padding={padding} border={focus_border(focused, Border::FOCUS)}>
            <Frame fill={line} />
        </Frame>
    }
}

#[component]
fn DockDropHighlight() -> NodeId {
    view! {
        <Layers>
            <Frame fill=ColorRole::Accent radius=Radius::Md opacity=DROP_OPACITY />
            <Frame @test_id={"dock.drop"} border={Border::FOCUS} radius=Radius::Md />
        </Layers>
    }
}

#[component]
fn DockDragPreview(title: Prop<String>, icon: Memo<Option<Glyph>>, grouped: bool) -> NodeId {
    let pictured = create_memo(move || !grouped && icon.get().is_some());
    let pictured_color = TextColor::Default;
    view! {
        <Frame
            opacity=PREVIEW_OPACITY
            fill=ColorRole::SurfaceRaised
            border={Border::FOCUS}
            radius=TAB_RADIUS
            height=TAB_HEIGHT
            padding={Padding::horizontal(PREVIEW_PADDING)}
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

fn bar_fill(vacant: Memo<bool>) -> Memo<ColorRole> {
    create_memo(move || match vacant.get() {
        true => ColorRole::Background,
        false => ColorRole::Surface,
    })
}

fn bar_rule(vacant: Memo<bool>) -> Memo<ColorRole> {
    create_memo(move || match vacant.get() {
        true => ColorRole::Background,
        false => ColorRole::Border,
    })
}
