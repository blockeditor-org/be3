use beui_macros::{component, view};

use crate::base::{Align, Direction, ItemSize};
use crate::color::Color32;
use crate::icons::{ICON_CLOSE, ICON_DRAG_INDICATOR, ICON_TAB_GROUP};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCallback, Frame, Func, List, Memo, Prop, ReadSignal, RenderFn, Show, Text,
    clone, create_memo,
};
use crate::styled::button::ButtonVariant;
use crate::styled::context_menu::ContextMenu;
use crate::styled::icon_button::{IconButton, IconButtonSize};
use crate::styled::text::{Body, IconSized};
use crate::styled::theme::{CARD_RADIUS, FONT_BODY, RADIUS, use_theme};
use crate::unstyled;
use crate::unstyled::{
    DockDragged, DockGripHandle, DockPanelHandle, DockPreviewHandle, DockSplitterHandle, DockState,
    DockTabHandle, DockWindowHandle, Entry, GroupId, MenuItem, SPLITTER_THICKNESS, TabId, sidebar_size,
};

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
const FOCUS_RING_WIDTH: f32 = 2.0;
pub(crate) const CHROME_BORDER: f32 = 2.0;
const GROUP_GLYPH: f32 = 16.0;
const GROUP_INSET: f32 = 6.0;
const DROP_ALPHA: u8 = 64;
const PREVIEW_ALPHA: u8 = 235;

#[component]
pub fn DockArea(
    state: Prop<DockState>,
    on_change: Callback<DockState>,
    on_close: Callback<TabId>,
    title: Func<TabId, String>,
    group_title: Option<Func<GroupId, Option<String>>>,
    closable: Option<Func<TabId, bool>>,
    #[prop(children)] content: RenderFn<TabId>,
) -> NodeId {
    let closable = closable.unwrap_or_else(|| Func::new(|_| true));
    let group_title = group_title.unwrap_or_else(|| Func::new(|_| None));
    view! {
        <unstyled::Dock
            state
            group_title
            group_inset=GROUP_INSET
            on_change={move |state: DockState| on_change.call(state)}
            on_close={move |tab: TabId| on_close.call(tab)}
            title
            content
            tab={clone!(closable -> move |handle: DockTabHandle| {
                let closable = closable.clone();
                view! {
                    <DockTabFace handle closable />
                }
            })}
            panel={clone!(closable -> move |handle: DockPanelHandle| {
                let closable = closable.clone();
                view! {
                    <DockPanelFace handle closable />
                }
            })}
            splitter={move |handle: DockSplitterHandle| view! {
                <DockSplitterFace handle />
            }}
            grip={move |handle: DockGripHandle| view! {
                <DockGrip handle />
            }}
            window={clone!(closable -> move |handle: DockWindowHandle| {
                let closable = closable.clone();
                view! {
                    <DockWindowFace handle closable />
                }
            })}
            highlight={move || view! {
                <DockDropHighlight />
            }}
            preview={move |handle: DockPreviewHandle| {
                let DockPreviewHandle { dragged, title } = handle;
                let grouped = !matches!(dragged, DockDragged::Entry(Entry::Tab(_)));
                view! {
                    <DockDragPreview title grouped />
                }
            }}
        />
    }
}

#[component]
fn DockTabFace(handle: DockTabHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockTabHandle {
        entry,
        title,
        tabs,
        has_next,
        selected,
        hovered,
        active,
        focused,
        dragged,
        close,
        float,
        group,
        split,
        ungroup,
        floating,
        pinned,
        vertical,
        held,
        toggle_held,
        ..
    } = handle;
    let stuck = create_memo(clone!(held -> move || floating || held.get() == Some(true)));
    let homeless = create_memo(clone!(held -> move || held.get().is_none()));
    let pin_label = create_memo(move || match held.get() {
        Some(true) => "Unpin from group".to_owned(),
        Some(false) | None => "Pin to group".to_owned(),
    });
    let closable = create_memo(move || {
        !pinned && tabs.with(|tabs| tabs.iter().all(|tab| closable.call(*tab)))
    });
    let alone = create_memo(clone!(has_next -> move || !has_next.get()));
    let grouped = matches!(entry, Entry::Group(_));
    let closing = close.clone();
    let items = match grouped {
        false => view! {
            <MenuItem label="Pop out into a window" disabled={stuck} />
            <MenuItem label="Group with next tab" disabled={alone.clone()} />
            <MenuItem label="Split with next tab" disabled={alone.clone()} />
            <MenuItem
                label="Close tab"
                disabled={create_memo(clone!(closable -> move || !closable.get()))}
            />
            <MenuItem label={pin_label} disabled={homeless} />
        },
        true => view! {
            <MenuItem label="Pop out into a window" disabled={floating} />
            <MenuItem label="Add next tab to group" disabled={alone.clone()} />
            <MenuItem label="Split next tab into group" disabled={alone.clone()} />
            <MenuItem
                label="Close group"
                disabled={create_memo(clone!(closable -> move || !closable.get()))}
            />
            <MenuItem label="Ungroup" disabled={pinned} />
        },
    };
    view! {
        <ContextMenu
            items={items}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => float.call(),
                Some(1) => group.call(),
                Some(2) => split.call(),
                Some(3) => closing.call(),
                Some(4) if grouped => ungroup.call(),
                Some(4) => toggle_held.call(),
                _ => {}
            }}
        >
            <DockTabChrome
                title
                grouped
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
        </ContextMenu>
    }
}

#[component]
fn DockTabChrome(
    title: Prop<String>,
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
            (true, _, false) => theme.surface.get(),
            (false, true, false) => theme.hover.get(),
            (false, false, false) => theme.background.get(),
        }
    }));
    let label = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.text.get(),
        false => theme.text_muted.get(),
    }));
    let glyph = label.clone();
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
            outline_visible={focused}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=TAB_SPACING>
                <Show condition={grouped}>
                    <IconSized glyph=ICON_TAB_GROUP font_size=GROUP_GLYPH color={glyph} />
                </Show>
                <Text
                    string={title}
                    font_size=FONT_BODY
                    color={label}
                    clip=true
                    @sizing={title_size}
                />
                <Show condition={closable}>
                    <IconButton
                        @test_id={close_test_id}
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
fn DockPanelFace(handle: DockPanelHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockPanelHandle {
        vertical,
        focused,
        contents,
        sidebar_width,
        sidebar_splitter,
        grip,
        bar,
        close,
        body,
        ..
    } = handle;
    let theme = use_theme();
    let closable = all_closable(contents, closable);
    view! {
        <List spacing=0.0>
            <Show condition={bar.is_some()}>
                <DockChrome
                    vertical
                    focused
                    grip={grip.unwrap_or_else(|| unreachable!())}
                    tabs={bar}
                    sidebar_width
                    sidebar_splitter
                    title=String::new()
                    closable
                    close={move || close.call()}
                    body
                    @sizing=ItemSize::Percent(100.0)
                />
            </Show>
            <Show condition={bar.is_none()}>
                <Frame color={theme.surface.clone()} @sizing=ItemSize::Percent(100.0)>{body}</Frame>
            </Show>
        </List>
    }
}

#[component]
fn DockWindowFace(handle: DockWindowHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockWindowHandle {
        vertical,
        focused,
        title,
        contents,
        sidebar_width,
        sidebar_splitter,
        grip,
        tabs,
        close,
        pane,
        ..
    } = handle;
    let closable = all_closable(contents, closable);
    view! {
        <DockChrome
            vertical
            focused
            grip
            tabs
            sidebar_width
            sidebar_splitter
            title
            closable
            close={move || close.call()}
            body={pane}
        />
    }
}

#[component]
fn DockChrome(
    vertical: bool,
    focused: Memo<bool>,
    grip: NodeId,
    #[prop(default = None)] tabs: Prop<Option<NodeId>>,
    sidebar_width: Memo<f32>,
    #[prop(default = None)] sidebar_splitter: Prop<Option<NodeId>>,
    title: Prop<String>,
    closable: Memo<bool>,
    close: ClickCallback,
    body: NodeId,
) -> NodeId {
    let sidebar_splitter = sidebar_splitter.peek();
    let sidebar = sidebar_size(vertical, sidebar_width);
    let theme = use_theme();
    let outline = create_memo(clone!(theme focused -> move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let direction = match vertical {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    };
    let tabs = tabs.peek();
    let side_title = title.clone();
    let side_closable = closable.clone();
    let side_close = close.clone();
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={outline}
            outline_width=CHROME_BORDER
            outline_visible=true
            radius=CARD_RADIUS
            padding_horizontal=CHROME_BORDER
            padding_vertical=CHROME_BORDER
        >
            <List direction spacing=0.0>
                <Show condition={vertical}>
                    <DockSideBar
                        grip
                        tabs
                        title={side_title}
                        closable={side_closable}
                        close={move || side_close.call()}
                        @sizing={sidebar}
                    />
                </Show>
                <Show condition={sidebar_splitter.is_some()}>
                    {sidebar_splitter.unwrap_or_else(|| unreachable!())} @sizing=ItemSize::Fixed(SPLITTER_THICKNESS)
                </Show>
                <Show condition={!vertical}>
                    <DockTitleBar grip tabs title closable close={move || close.call()} />
                </Show>
                {body} @sizing=ItemSize::Percent(100.0)
            </List>
        </Frame>
    }
}

#[component]
fn DockTitleBar(
    grip: NodeId,
    #[prop(default = None)] tabs: Prop<Option<NodeId>>,
    title: Prop<String>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let tabs = tabs.peek();
    let titled = tabs.is_none();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_vertical=BAR_PADDING
            padding_horizontal=BAR_PADDING
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                {grip}
                <Show condition={!titled}>
                    {tabs.unwrap_or_else(|| unreachable!())} @sizing=ItemSize::Percent(100.0)
                </Show>
                <Show condition={titled}>
                    <Body content={title} @sizing=ItemSize::Percent(100.0) />
                </Show>
                <DockClose closable close={move || close.call()} />
            </List>
        </Frame>
    }
}

#[component]
fn DockSideBar(
    grip: NodeId,
    #[prop(default = None)] tabs: Prop<Option<NodeId>>,
    title: Prop<String>,
    closable: Memo<bool>,
    close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let tabs = tabs.peek();
    let titled = tabs.is_none();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_vertical=BAR_PADDING
            padding_horizontal=BAR_PADDING
        >
            <List spacing=BAR_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    {grip}
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <DockClose closable close={move || close.call()} />
                </List>
                <Show condition={!titled}>
                    {tabs.unwrap_or_else(|| unreachable!())} @sizing=ItemSize::Percent(100.0)
                </Show>
                <Show condition={titled}>
                    <Body content={title} />
                </Show>
            </List>
        </Frame>
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
    let DockGripHandle {
        focused,
        vertical,
        toggle_vertical,
        ..
    } = handle;
    let theme = use_theme();
    let color = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.text_muted.get(),
        false => theme.border.get(),
    }));
    view! {
        <ContextMenu
            items={view! {
                <MenuItem label={orientation_label(vertical)} />
            }}
            on_select={move |_: Vec<usize>| toggle_vertical.call()}
        >
            <Frame
                width=GRIP_WIDTH
                padding_vertical=WINDOW_BAR_PADDING
                padding_horizontal=GRIP_PADDING
            >
                <IconSized glyph=ICON_DRAG_INDICATOR font_size=GRIP_GLYPH color={color} />
            </Frame>
        </ContextMenu>
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
fn DockDragPreview(title: Prop<String>, grouped: bool) -> NodeId {
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

fn all_closable(contents: Memo<Vec<TabId>>, closable: Func<TabId, bool>) -> Memo<bool> {
    create_memo(move || {
        contents.with(|tabs| !tabs.is_empty() && tabs.iter().all(|tab| closable.call(*tab)))
    })
}

fn orientation_label(vertical: bool) -> &'static str {
    match vertical {
        true => "Show tabs across the top",
        false => "Show tabs in a sidebar",
    }
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}
