use std::time::Duration;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{ICON_CLOSE, ICON_NOTIFICATIONS, ICON_NOTIFICATIONS_ACTIVE};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, Show, Spacer, Text, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::theme::{FONT_BODY, FONT_SMALL};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, Heading, IconButton, IconButtonSize, ListRow, Scroll, use_theme,
};
use block_editor_beui::beui::unstyled::PopoverHandle;

use super::{DesktopNotifications, Listed};
use crate::app::bar::local_minutes;
use crate::app::popup::BarPopup;

pub(crate) const NOTIFICATIONS_WIDTH: f32 = 360.0;
const LIST_HEIGHT: f32 = 420.0;
const SPACING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;

fn received_at(received: u64) -> String {
    let minutes = local_minutes(Duration::from_secs(received));
    Time::from_minutes(minutes).format(HourCycle::H24)
}

#[component]
pub(crate) fn NotificationsButton(notifications: DesktopNotifications) -> NodeId {
    let listed = notifications.listed();
    let browsing = notifications.clone();
    let glyph = create_memo(clone!(listed -> move || {
        match listed.with(Vec::is_empty) {
            true => ICON_NOTIFICATIONS.to_owned(),
            false => ICON_NOTIFICATIONS_ACTIVE.to_owned(),
        }
    }));
    let label = create_memo(clone!(listed -> move || {
        match listed.with(Vec::len) {
            0 => "Notifications".to_owned(),
            count => format!("Notifications ({count})"),
        }
    }));
    view! {
        <BarPopup
            label
            glyph
            icon_only=true
            width=NOTIFICATIONS_WIDTH
            @test_id={"desktop.notifications"}
            on_open_change={move |open: bool| browsing.browse(open)}
        >
            {move |handle: PopoverHandle| view! {
                <NotificationList
                    notifications={notifications.clone()}
                    listed={listed.clone()}
                    popover={handle}
                />
            }}
        </BarPopup>
    }
}

#[component]
fn NotificationList(
    notifications: DesktopNotifications,
    listed: Memo<Vec<Listed>>,
    popover: PopoverHandle,
) -> NodeId {
    let empty = create_memo(clone!(listed -> move || listed.with(Vec::is_empty)));
    let unclearable = empty.clone();
    let some = create_memo(clone!(empty -> move || !empty.get()));
    let clearing = notifications.clone();
    let cleared = listed.clone();
    let clear = move || {
        let ids: Vec<u32> =
            cleared.with_untracked(|listed| listed.iter().map(|shown| shown.id).collect());
        if !ids.is_empty() {
            clearing.dismiss(&ids);
        }
    };
    view! {
        <List spacing=SPACING @test_id={"desktop.notifications.list"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Heading content="Notifications" />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Button
                    label="Clear all"
                    variant=ButtonVariant::Ghost
                    disabled={unclearable}
                    @test_id={"desktop.notifications.clear"}
                    on_click={clear}
                />
            </List>
            <Show condition={empty}>
                <Caption content="Nothing new." />
            </Show>
            <Show condition={some}>
                <NotificationRows
                    notifications={notifications.clone()}
                    listed={listed.clone()}
                    popover={popover.clone()}
                />
            </Show>
        </List>
    }
}

#[component]
fn NotificationRows(
    notifications: DesktopNotifications,
    listed: Memo<Vec<Listed>>,
    popover: PopoverHandle,
) -> NodeId {
    view! {
        <Scroll max_length=LIST_HEIGHT>
            <ForEach keys={listed}>
                {move |shown: Listed| view! {
                    <NotificationRow
                        notifications={notifications.clone()}
                        shown
                        popover={popover.clone()}
                    />
                }}
            </ForEach>
        </Scroll>
    }
}

#[component]
fn NotificationRow(
    notifications: DesktopNotifications,
    shown: Listed,
    popover: PopoverHandle,
) -> NodeId {
    let theme = use_theme();
    let id = shown.id;
    let activates = shown.activates;
    let dismissing = notifications.clone();
    let activate = move || match activates {
        true => {
            notifications.activate(id);
            popover.close.call(());
        }
        false => notifications.dismiss(&[id]),
    };
    let source = match shown.app_name.is_empty() {
        true => received_at(shown.received),
        false => format!("{}  {}", shown.app_name, received_at(shown.received)),
    };
    let said = !shown.body.is_empty();
    let body = shown.body.clone();
    let summary = shown.summary.clone();
    let body_color = theme.text_muted.clone();
    let summary_color = match shown.critical {
        true => theme.danger.clone(),
        false => theme.text.clone(),
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Start spacing=ROW_SPACING>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("desktop.notifications.{id}")}
                on_click={activate}
            >
                <List spacing=ROW_SPACING>
                    <Caption content={source} />
                    <Text
                        string={summary}
                        font_size=FONT_BODY
                        color={summary_color}
                        bold=true
                        wrap=true
                    />
                    <Show condition={said}>
                        <Text
                            string={body.clone()}
                            font_size=FONT_SMALL
                            color={body_color.clone()}
                            wrap=true
                        />
                    </Show>
                </List>
            </ListRow>
            <IconButton
                glyph={ICON_CLOSE.to_owned()}
                label="Dismiss"
                variant=ButtonVariant::Ghost
                size=IconButtonSize::Compact
                @test_id={format!("desktop.notifications.{id}.dismiss")}
                on_click={move || dismissing.dismiss(&[id])}
            />
        </List>
    }
}
