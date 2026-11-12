use beui_view::types::AboveEmbeds;
use beui_view::types::Overflow;
use beui_view::types::icons::Glyph;
mod stack;
mod switch;

use beui_components_styled::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};

use crate::unstyled as dock;
pub use crate::unstyled::{
    DockApi, DockArea, DockHostLayout, DockSnap, DockWindowActions, HiddenBand,
};
use crate::unstyled::{
    DockAwayHandle, DockBarBandHandle, DockBarHandle, DockChromeHandle, DockDialogHandle,
    DockDragged, DockGripHandle, DockHighlightHandle, DockPreviewHandle, DockSplitterHandle,
    DockStackHandle, DockStripHandle, DockSwitchHandle, DockSwitcherHandle, DockTabHandle,
};
use beui_components_styled::action_bar::{ActionBar, ActionLabels};
use beui_components_styled::button::{Button, ButtonSize, ButtonVariant};
use beui_components_styled::context_menu::menu_style;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize};
use beui_components_styled::theme::{
    NARROW_WIDTH, PHONE_LANDSCAPE_WIDTH, POINTER_REACH, SHORT_HEIGHT, use_theme,
};
use beui_components_styled::tokens::{Padding, Size, Space, Stroke};
use beui_components_styled::tool_pill::BAND_FILL;
use beui_components_styled::tooltip::Tooltip;
use beui_components_styled::{Length, List, Scroll, Separator};
use beui_components_unstyled::Container;
use beui_components_unstyled::Picture;
use beui_dock::{DockEdge, DockHostLink, DockMode, Entry};
use beui_view::component_ref::ComponentRef;
use beui_view::primitives::Interactive;
use beui_view::types::Align2;
use beui_view::types::PointerPress;
use beui_view::types::Vec2;
use beui_view::types::{Align, Direction, Justify};
use std::rc::Rc;

use beui_view::primitives::Layers;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Modifiers;
use beui_view::types::TextAlign;
use beui_view::types::icons::{
    ICON_APP_WINDOW, ICON_GRIP_VERTICAL, ICON_LOCK, ICON_MAXIMIZE, ICON_MINIMIZE, ICON_X,
};
use stack::{DockStackBar, DockSwitcher};
use switch::DockSwitchPanel;

const TAB_PADDING_HORIZONTAL: Space = Space::Md;
const TAB_HEIGHT: Size = Size::Toolbar;
const TAB_RADIUS: Radius = Radius::Tab;
const CARD_RADIUS: Radius = Radius::Lg;
const TAB_SPACING: Space = Space::Md;
const BAR_HEIGHT: f32 = 39.0;
const ROOMY_BAR: f32 = 400.0;
const BAR_PADDING: Space = Space::Xs;
const BAR_SPACING: Space = Space::Xs2;
const BAR_MARGIN: Space = Space::Md;
const GRIP_WIDTH: Size = Size::Xs;
const ICON_SIDEBAR: f32 = TAB_HEIGHT.px() + 2.0 * BAR_PADDING.px();
const PREVIEW_PADDING: Space = Space::Md;
pub const CHROME_BORDER: f32 = CHROME_STROKE.px();
const CHROME_STROKE: Stroke = Stroke::Focus;
const CHROME_PADDING: Space = Space::Xs2;
const RULE: f32 = 1.0;
const SPLITTER_LINE: f32 = 1.0;
const TAB_STRIP_SPACING: Space = Space::Sm;
const SPLITTER_REACH: f32 = 3.0;
const SPLITTER_HELD: f32 = 3.0;
const GROUP_GLYPH: Size = Size::Xs;
pub const WINDOW_CHROME: beui_view::types::Vec2 =
    beui_view::types::Vec2::new(2.0 * CHROME_BORDER, BAR_HEIGHT + 2.0 * CHROME_BORDER + RULE);
const DROP_OPACITY: f32 = 0.25;
const DROP_LABEL_MARGIN: Space = Space::Md;
const DROP_LABEL_PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs);
const AWAY_SPACING: Space = Space::Lg;
const AWAY_PADDING: Space = Space::Xl3;
const PREVIEW_OPACITY: f32 = 0.92;
const WAITING_DOT: Size = Size::Custom(6.0);

#[component]
pub fn DockHost(
    #[prop(default = DockHostLayout::new())] layout: DockHostLayout,
    #[prop(default = ComponentRef::new())] api: ComponentRef<DockApi>,
    bars: Option<Rc<dyn DockHostLink>>,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    #[prop(children)] content: Render,
) -> NodeId {
    let mode = layout.mode();
    let theme = use_theme();
    let back_shade = create_memo(move || ColorRole::ScrimSoft.resolve(&theme));
    let through = beui_dock::dock_shows_through();
    let fill = create_memo(move || match mode.get() {
        DockMode::Stacked if !through.get() => ColorRole::Background,
        DockMode::Tiled | DockMode::Compact | DockMode::Stacked => ColorRole::None,
    });
    view! {
        <dock::DockHost
            layout
            api
            bars
            stack_below=NARROW_WIDTH
            short_below=SHORT_HEIGHT
            short_narrower_than=PHONE_LANDSCAPE_WIDTH
            drag_modifier
            splitter_thickness=SPLITTER_LINE
            splitter_reach=SPLITTER_REACH
            icon_sidebar_width=ICON_SIDEBAR
            tab_spacing={TAB_STRIP_SPACING.px()}
            back_shade={back_shade}
            menu={menu_style()}
            content={move || content.call(())}
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
            dialog_beak=POINTER_REACH
            dialog_radius={CARD_RADIUS.px()}
            dialog_scrim={create_memo(move || ColorRole::Scrim.resolve(&theme))}
            window_border=CHROME_BORDER
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
            strip={move |handle: DockStripHandle| view! {
                <DockStrip handle />
            }}
            grip={move |handle: DockGripHandle| view! {
                <DockGrip handle />
            }}
            highlight={move |handle: DockHighlightHandle| view! {
                <DockDropHighlight handle />
            }}
            bar_margin={BAR_MARGIN.px()}
            bar_band={move |handle: DockBarBandHandle| view! {
                <DockBarBandFace handle />
            }}
            refused={move || view! {
                <DockDropRefused />
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
                    on_click={move || leave.call(())}
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
        test_key,
        grouped,
        ..
    } = handle;
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
                            waiting_test_id={waiting_test_id(entry, test_key.as_deref())}
                            close_test_id={close_test_id(test_key.as_deref())}
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
            (true, _, false) => ColorRole::Control,
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
    close: Callback<()>,
) -> NodeId {
    let fill = tab_fill(selected, hovered, active, dragged, noticed);
    let label = create_memo(move || match (noticed.get(), selected.get()) {
        (true, _) => TextColor::Accent,
        (false, true) => TextColor::Default,
        (false, false) => TextColor::Muted,
    });
    let shows_picture = create_memo(move || !grouped && picture.with(Option::is_some));
    let pictured = create_memo(move || !grouped && !shows_picture.get() && icon.get().is_some());
    let (title_grow, title_shrink) = match vertical {
        true => (1.0, 0.0),
        false => (0.0, 1.0),
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
                <ListItem grow=title_grow shrink=title_shrink>
                    <Text color={label} overflow=Overflow::Ellipsis>{title}</Text>
                </ListItem>
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
                        size=ButtonSize::Inline
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

fn dialog_outline(focused: Memo<bool>, attention: Memo<bool>) -> Memo<Option<Border>> {
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
fn DockDialogFace(handle: DockDialogHandle) -> NodeId {
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
        grab,
        drag,
        card,
        header,
        body,
        ..
    } = handle;
    let pictured = create_memo(move || icon.get().is_some());
    let width = create_memo(move || Size::Custom(size.get().x + 2.0 * CHROME_BORDER));
    let header_fill = create_memo(move || match notice.with(Option::is_some) {
        true => ColorRole::AccentSoft,
        false => ColorRole::None,
    });
    let label = create_memo(move || match notice.with(Option::is_some) {
        true => TextColor::Accent,
        false => TextColor::Default,
    });
    let title = create_memo(move || notice.get().unwrap_or_else(|| title.get()));
    view! {
        <Frame
            @node_ref=&card
            width=width
            fill=ColorRole::Surface
            border={dialog_outline(focused, create_memo(move || attention.get()))}
            radius=CARD_RADIUS
            padding=CHROME_PADDING
        >
            <List spacing=Space::None>
                <Interactive
                    @node_ref=&header
                    @test_id={format!("dock.dialog.{}.header", tab.value())}
                    cursor=beui_view::types::CursorIcon::Move
                    on_press={move |press: PointerPress| grab.call(press.pos)}
                    on_drag={move |press: PointerPress| drag.call(press.pos)}
                >
                    <Frame
                        fill={header_fill}
                        radius=Radius::Md
                        height=TAB_HEIGHT
                        padding={Padding::horizontal(TAB_PADDING_HORIZONTAL)}
                    >
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TAB_SPACING
                        >
                            <Show condition={pictured}>
                                <Text
                                    size=TextSize::Sm
                                    color=TextColor::Muted
                                    align=TextAlign::Center
                                >
                                    <Icon glyph={icon} />
                                </Text>
                            </Show>
                            <ListItem grow=1.0>
                                <Text color={label} overflow=Overflow::Ellipsis>{title}</Text>
                            </ListItem>
                            <Show condition={closable}>
                                <Button
                                    icon_only=true
                                    @test_id={format!("dock.dialog.{}.close", tab.value())}
                                    glyph=ICON_X
                                    label="Close"
                                    size=ButtonSize::Inline
                                    variant=ButtonVariant::Ghost
                                    capture_presses=true
                                    on_click={close}
                                />
                            </Show>
                        </List>
                    </Frame>
                </Interactive>
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
        test_prefix,
        ..
    } = handle;
    let close = move || close.call(());
    match vertical {
        true => view! {
            <DockSideBar grip tabs title vacant menu closable icons_only close test_prefix />
        },
        false => view! {
            <DockTitleBar grip tabs title vacant menu closable close test_prefix />
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
    close: Callback<()>,
    test_prefix: String,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let rule = bar_rule(vacant);
    let trailing = create_memo(move || {
        let actions = closable.get() || menu.with(|menu| !menu.is_empty());
        Padding::new(
            match actions && titled {
                true => BAR_SPACING,
                false => Space::None,
            },
            Space::None,
            match actions || titled {
                true => BAR_PADDING,
                false => Space::None,
            },
            Space::None,
        )
    });
    view! {
        <List spacing=Space::None>
            <Frame
                fill={fill}
                height={Size::Custom(BAR_HEIGHT)}
                padding={Padding::new(BAR_PADDING, Space::None, Space::None, Space::None)}
                align={Align2::new(Align::Stretch, Align::Center)}
            >
                <Container>|size: ReadSignal<Vec2>| {
                    let room = create_memo(move || match size.get().x < ROOMY_BAR {
                        true => BarRoom::Narrow,
                        false => BarRoom::Wide,
                    });
                    let menu_prefix = test_prefix.clone();
                    view! {
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=BAR_SPACING
                        >
                            {grip}
                            {tabs.map(|tabs| view! {
                                <ListItem grow=1.0 gap=0.0>{tabs}</ListItem>
                            })}
                            <Show condition={titled}>|| {
                                let title = title.clone(); view! {
                                    <ListItem grow=1.0>
                                        <Text overflow=Overflow::Ellipsis>{title}</Text>
                                    </ListItem>
                                }
                            }</Show>
                            <ListItem gap=0.0>
                                <Frame padding={trailing}>
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=BAR_SPACING
                                    >
                                        <Dynamic value={room}>|room: BarRoom|
                                            <DockMenu menu room test_prefix={menu_prefix.clone()} />
                                        </Dynamic>
                                        <DockClose
                                            closable
                                            close={close}
                                            test_prefix={test_prefix.clone()}
                                        />
                                    </List>
                                </Frame>
                            </ListItem>
                        </List>
                    }
                }</Container>
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
    close: Callback<()>,
    test_prefix: String,
) -> NodeId {
    let titled = tabs.is_none();
    let fill = bar_fill(vacant);
    let titles = create_memo(move || !icons_only.get());
    let header = create_memo(move || match icons_only.get() {
        true => Justify::Center,
        false => Justify::SpaceBetween,
    });
    let narrow = test_prefix.clone();
    let column = test_prefix;
    let trailing = create_memo(move || match titled || icons_only.get() {
        true => Padding::horizontal(BAR_PADDING).bottom(BAR_PADDING),
        false => Padding::horizontal(BAR_PADDING),
    });
    let after_tabs_gap = match titled {
        true => None,
        false => Some(0.0),
    };
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <ListItem grow=1.0>
                <Frame fill={fill}>
                    <List spacing=BAR_SPACING>
                        <Frame padding={Padding::all(BAR_PADDING).bottom(Space::None)}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                justify={header}
                                spacing=BAR_SPACING
                            >
                                {grip}
                                <Show condition={titles}>|| {
                                    let test_prefix = narrow.clone(); view! {
                                        <List
                                            direction=Direction::Horizontal
                                            align=Align::Center
                                            spacing=BAR_SPACING
                                        >
                                            <DockMenu
                                                menu
                                                room=BarRoom::Narrow
                                                test_prefix={test_prefix.clone()}
                                            />
                                            <DockClose closable close={close} test_prefix />
                                        </List>
                                    }
                                }</Show>
                            </List>
                        </Frame>
                        {tabs.map(|tabs| view! {
                            <ListItem grow=1.0 gap=0.0>{tabs}</ListItem>
                        })}
                        <ListItem gap=after_tabs_gap>
                            <Frame padding={trailing}>
                                <List spacing=BAR_SPACING>
                                    <Show condition={titled}>|| {
                                        let title = title.clone(); view! {
                                            <Text overflow=Overflow::Ellipsis>{title}</Text>
                                        }
                                    }</Show>
                                    <Show condition={icons_only}>|| {
                                        let test_prefix = column.clone(); view! {
                                            <List align=Align::Center spacing=BAR_SPACING>
                                                <DockMenu
                                                    menu
                                                    room=BarRoom::Column
                                                    test_prefix={test_prefix.clone()}
                                                />
                                                <DockClose closable close={close} test_prefix />
                                            </List>
                                        }
                                    }</Show>
                                </List>
                            </Frame>
                        </ListItem>
                    </List>
                </Frame>
            </ListItem>
        </List>
    }
}

#[component]
fn DockStrip(handle: DockStripHandle) -> NodeId {
    let DockStripHandle { direction, tabs } = handle;
    let strip = match direction {
        Direction::Horizontal => view! {
            <Frame
                height={Size::Custom(BAR_HEIGHT)}
                padding={Padding::new(BAR_SPACING, Space::None, BAR_PADDING, Space::None)}
                align={Align2::new(Align::Start, Align::Center)}
            >
                {tabs}
            </Frame>
        },
        Direction::Vertical => view! {
            <Frame padding={Padding::new(BAR_PADDING, BAR_SPACING, BAR_PADDING, BAR_PADDING)}>
                {tabs}
            </Frame>
        },
    };
    view! {
        <Scroll direction>
            <ListItem shrink=1.0>{strip}</ListItem>
        </Scroll>
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
pub(crate) fn DockMenu(menu: Memo<Vec<MenuNode>>, room: BarRoom, test_prefix: String) -> NodeId {
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
        <ActionBar menu labels title={room == BarRoom::Phone} direction size test_prefix />
    }
}

#[component]
fn DockClose(
    closable: Memo<bool>,
    close: Callback<()>,
    test_prefix: String,
) -> DynamicSegment<ListItem> {
    view! {
        <Show condition={closable}>
            <Button
                icon_only=true
                @test_id={format!("{test_prefix}.close")}
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
    let DockGripHandle {
        focused, locked, ..
    } = handle;
    let color = create_memo(move || match focused.get() {
        true => TextColor::Muted,
        false => TextColor::Faint,
    });
    let glyph = create_memo(move || match locked.get() {
        true => Some(ICON_LOCK),
        false => Some(ICON_GRIP_VERTICAL),
    });
    view! {
        <Frame width=GRIP_WIDTH height=TAB_HEIGHT align=Align::Center>
            <Text size=TextSize::Sm color={color} align=TextAlign::Center>
                <Icon glyph />
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
    let border = focus_border(focused, Border::FOCUS);
    let line = view! {
        <Frame fill={line} />
    };
    above_embeds(line, Some(AboveEmbeds::Paints));
    let ring = view! {
        <Frame padding={padding} border={border}>{line}</Frame>
    };
    above_embeds(
        ring,
        create_memo(move || border.get().map(|_| AboveEmbeds::Paints)),
    );
    ring
}

#[component]
fn DockBarBandFace(handle: DockBarBandHandle) -> NodeId {
    let DockBarBandHandle { edge, body } = handle;
    let along = edge.direction();
    let across = match along {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    };
    let held = view! {
        <Frame>{body}</Frame>
    };
    let line = view! {
        <Separator direction=along length=Length::FILL />
    };
    let parts = match edge {
        DockEdge::Top | DockEdge::Left => [held, line],
        DockEdge::Bottom | DockEdge::Right => [line, held],
    };
    let [first, second] = parts;
    view! {
        <Frame fill=BAND_FILL @test_id={format!("dock.bar.band.{}", edge.name())}>
            <List direction=across spacing=Space::None>{first}{second}</List>
        </Frame>
    }
}

#[component]
fn DockDropHighlight(handle: DockHighlightHandle) -> NodeId {
    let DockHighlightHandle { group_with } = handle;
    let grouping = create_memo(move || group_with.get().is_some());
    let label = create_memo(move || {
        group_with
            .get()
            .map(|title| format!("Group with {title}"))
            .unwrap_or_default()
    });
    view! {
        <Layers>
            <Frame fill=ColorRole::Accent radius=Radius::Md opacity=DROP_OPACITY />
            <Frame @test_id={"dock.drop"} border={Border::FOCUS} radius=Radius::Md />
            <Show condition={grouping}>
                <Frame align=Align2::CENTER padding=DROP_LABEL_MARGIN>
                    <Frame
                        fill=ColorRole::Accent
                        radius=Radius::Md
                        padding=DROP_LABEL_PADDING
                        @test_id="dock.drop.label"
                    >
                        <Text color=TextColor::OnAccent overflow=Overflow::Ellipsis>{label}</Text>
                    </Frame>
                </Frame>
            </Show>
        </Layers>
    }
}

#[component]
fn DockDropRefused() -> NodeId {
    view! {
        <Layers>
            <Frame fill=ColorRole::Surface radius=Radius::Md opacity=DROP_OPACITY />
            <Frame
                @test_id={"dock.refused"}
                border={Border::DEFAULT}
                radius=Radius::Md
                align=Align::Center
            >
                <Text size=TextSize::Lg color=TextColor::Muted>
                    <Icon glyph=ICON_LOCK />
                </Text>
            </Frame>
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
            fill=ColorRole::Control
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

fn waiting_test_id(entry: Entry, key: Option<&str>) -> String {
    match (entry, key) {
        (Entry::Tab(_), Some(key)) => format!("dock.tab.{key}.waiting"),
        _ => String::new(),
    }
}

fn close_test_id(key: Option<&str>) -> String {
    key.map_or_else(String::new, |key| format!("dock.tab.{key}.close"))
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
