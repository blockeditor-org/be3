use std::time::Duration;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{ICON_BELL, ICON_BELL_RING, ICON_X};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::{DesktopNotifications, Listed};
use crate::app::bar::local_minutes;
use crate::app::popup::BarPopup;

const LIST_HEIGHT: f32 = 420.0;
const SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;

fn received_at(received: u64) -> String {
    let minutes = local_minutes(Duration::from_secs(received));
    Time::from_minutes(minutes).format(HourCycle::H24)
}

#[component]
pub(crate) fn NotificationsButton(notifications: DesktopNotifications) -> NodeId {
    let listed = notifications.listed();
    let glyph = create_memo(move || match listed.with(Vec::is_empty) {
        true => ICON_BELL,
        false => ICON_BELL_RING,
    });
    let label = create_memo(move || match listed.with(Vec::len) {
        0 => "Notifications".to_owned(),
        count => format!("Notifications ({count})"),
    });
    view! {
        <BarPopup
            label
            glyph
            icon_only=true
            @test_id={"desktop.notifications"}
            open={notifications.browsing()}
        >|handle: PopoverHandle|
            <NotificationList notifications={notifications} listed={listed} popover={handle} />
        </BarPopup>
    }
}

#[component]
fn NotificationList(
    notifications: DesktopNotifications,
    listed: Memo<Vec<Listed>>,
    popover: PopoverHandle,
) -> NodeId {
    let empty = create_memo(move || listed.with(Vec::is_empty));
    let some = create_memo(move || !empty.get());
    let clear = move || {
        let ids: Vec<u32> =
            listed.with_untracked(|listed| listed.iter().map(|shown| shown.id).collect());
        if !ids.is_empty() {
            notifications.dismiss(&ids);
        }
    };
    view! {
        <List spacing=SPACING @test_id={"desktop.notifications.list"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text style=TextStyle::HEADING>"Notifications"</Text>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Button
                    label="Clear all"
                    variant=ButtonVariant::Ghost
                    disabled={empty}
                    @test_id={"desktop.notifications.clear"}
                    on_click={clear}
                />
            </List>
            <Show condition={empty}>
                <Text style=TextStyle::CAPTION>"Nothing new."</Text>
            </Show>
            <Show condition={some}>
                <NotificationRows notifications={notifications} listed={listed} popover={popover} />
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
            <ForEach keys={listed}>|shown: Listed|
                <NotificationRow notifications={notifications} shown popover={popover} />
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
    let id = shown.id;
    let activates = shown.activates;
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
    let body_color = TextColor::Muted;
    let summary_color = match shown.critical {
        true => TextColor::Danger,
        false => TextColor::Default,
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Start spacing=ROW_SPACING>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("desktop.notifications.{id}")}
                on_click={activate}
            >
                <List spacing=ROW_SPACING>
                    <Text style=TextStyle::CAPTION>{source}</Text>
                    <Text color={summary_color} weight=Weight::Bold>{summary}</Text>
                    <Show condition={said}>|| {
                        let body = body.clone(); view! {
                            <Text size=TextSize::Sm color={body_color}>{body}</Text>
                        }
                    }</Show>
                </List>
            </ListRow>
            <Button
                icon_only=true
                glyph=ICON_X
                label="Dismiss"
                variant=ButtonVariant::Ghost
                size=ButtonSize::Small
                @test_id={format!("desktop.notifications.{id}.dismiss")}
                on_click={move || notifications.dismiss(&[id])}
            />
        </List>
    }
}
