use beui_macros::{component, view};

use crate::action_row::ActionRow;
use crate::border::Separator;
use crate::button::ButtonVariant;
use crate::icon_button::IconButton;
use crate::list_row::ListRow;
use crate::scroll::Scroll;
use crate::sheet::{ModalSheet, SHEET_STOPS};
use crate::text::{Caption, Heading, Icon};
use crate::theme::{FONT_BODY, use_theme};
use beui_components_unstyled::{DockStackHandle, TabId};
use beui_core::base::{Align, Direction, ItemSize};
use beui_core::icons::{ICON_ARROW_BACK, ICON_CLOSE, ICON_EXPAND_MORE, ICON_HOME};
use beui_core::node::NodeId;
use beui_view::reactive::{
    ForEach, Frame, Func, List, Memo, Portal, Show, Text, WriteSignal, clone, create_memo,
    create_signal,
};

const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 2.0;
const TITLE_PADDING: f32 = 6.0;
const TITLE_SPACING: f32 = 6.0;
const SHEET_PADDING: f32 = 12.0;
const ROW_SPACING: f32 = 2.0;

#[component]
pub(crate) fn DockStackBar(handle: DockStackHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockStackHandle {
        shown,
        title,
        home,
        away,
        tabs,
        actions,
        titles,
        back,
        show,
        close,
    } = handle;
    let theme = use_theme();
    let (open, set_open) = create_signal(false);
    let opening = set_open.clone();
    let closing = set_open.clone();
    view! {
        <List spacing=0.0>
            <Frame
                color={theme.surface.clone()}
                padding_horizontal=BAR_PADDING
                padding_vertical=BAR_PADDING
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Show condition={away}>
                        <IconButton
                            @test_id={"dock.back"}
                            glyph=ICON_ARROW_BACK
                            label="Back"
                            variant=ButtonVariant::Ghost
                            on_click={move || back.call()}
                        />
                    </Show>
                    <ListRow
                        @sizing=ItemSize::Percent(100.0)
                        @test_id={"dock.switch"}
                        on_click={move || opening.set(true)}
                    >
                        <Frame padding_horizontal=TITLE_PADDING>
                            <List direction=Direction::Horizontal align=Align::Center spacing=TITLE_SPACING>
                                <Text
                                    @sizing=ItemSize::Percent(100.0)
                                    string={title}
                                    font_size=FONT_BODY
                                    color={theme.text.clone()}
                                    ellipsis=true
                                />
                                <Icon glyph=ICON_EXPAND_MORE color={theme.text_muted.clone()} />
                            </List>
                        </Frame>
                    </ListRow>
                    <Portal node={actions} />
                </List>
            </Frame>
            <Separator />
            <ModalSheet open={open} rest={SHEET_STOPS[2]} on_close={move || closing.set(false)}>
                <DockSwitcher shown home tabs titles show close closable set_open />
            </ModalSheet>
        </List>
    }
}

#[component]
fn DockSwitcher(
    shown: Memo<Option<TabId>>,
    home: Memo<Option<TabId>>,
    tabs: Memo<Vec<TabId>>,
    titles: Func<TabId, String>,
    show: Func<TabId, ()>,
    close: Func<TabId, ()>,
    closable: Func<TabId, bool>,
    set_open: WriteSignal<bool>,
) -> NodeId {
    let none = create_memo(clone!(tabs -> move || tabs.get().is_empty()));
    let homed = create_memo(clone!(home -> move || home.get().is_some()));
    let home_titles = titles.clone();
    let home_title = create_memo(clone!(home -> move || {
        home.get().map(|home| home_titles.call(home)).unwrap_or_default()
    }));
    let going = show.clone();
    let leaving = set_open.clone();
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=SHEET_PADDING>
                <Heading content="Open tabs" />
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <List spacing=ROW_SPACING>
                        <Show condition={none}>
                            <Caption content="Nothing else is open." />
                        </Show>
                        <ForEach keys={tabs}>
                            {move |tab: TabId| {
                                let shown = shown.clone();
                                let titles = titles.clone();
                                let show = show.clone();
                                let close = close.clone();
                                let closable = closable.call(tab);
                                let set_open = set_open.clone();
                                view! {
                                    <DockSwitcherRow tab shown titles show close closable set_open />
                                }
                            }}
                        </ForEach>
                    </List>
                </Frame>
            </Scroll>
            <Show condition={homed}>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <ActionRow
                        @test_id={"dock.switcher.home"}
                        label={home_title}
                        glyph=ICON_HOME
                        on_click={move || {
                            leaving.set(false);
                            if let Some(home) = home.get_untracked() {
                                going.call(home);
                            }
                        }}
                    />
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn DockSwitcherRow(
    tab: TabId,
    shown: Memo<Option<TabId>>,
    titles: Func<TabId, String>,
    show: Func<TabId, ()>,
    close: Func<TabId, ()>,
    closable: bool,
    set_open: WriteSignal<bool>,
) -> NodeId {
    let theme = use_theme();
    let title = create_memo(move || titles.call(tab));
    let current = create_memo(move || shown.get() == Some(tab));
    let id = tab.value();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("dock.switcher.tab.{id}")}
                selected={current}
                on_click={move || {
                    set_open.set(false);
                    show.call(tab);
                }}
            >
                <Text
                    string={title}
                    font_size=FONT_BODY
                    color={theme.text.clone()}
                    ellipsis=true
                />
            </ListRow>
            <Show condition={closable}>
                <IconButton
                    @test_id={format!("dock.switcher.close.{id}")}
                    glyph=ICON_CLOSE
                    label="Close tab"
                    variant=ButtonVariant::Ghost
                    on_click={move || close.call(tab)}
                />
            </Show>
        </List>
    }
}
