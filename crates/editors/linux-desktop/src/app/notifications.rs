use std::time::Duration;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{ICON_CLOSE, ICON_NOTIFICATIONS, ICON_NOTIFICATIONS_ACTIVE};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, Text, clone,
    component, create_memo, view,
};
use block_editor_beui::beui::styled::theme::{FONT_BODY, FONT_SMALL};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, Heading, IconButton, IconButtonSize, ListRow, Scroll,
    use_theme,
};
use block_editor_beui::beui::unstyled::PopoverHandle;
use block_editor_beui::{Editor, HostNotification};

use super::bar::local_minutes;
use super::popup::BarPopup;

pub(crate) const NOTIFICATIONS_WIDTH: f32 = 360.0;
const LIST_HEIGHT: f32 = 420.0;
const SPACING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;

fn received_at(received: u64) -> String {
    let minutes = local_minutes(Duration::from_secs(received));
    Time::from_minutes(minutes).format(HourCycle::H24)
}

#[component]
pub(crate) fn NotificationsButton(editor: Editor) -> NodeId {
    let notifications = editor.notifications();
    let glyph = create_memo(clone!(notifications -> move || {
        match notifications.with(Vec::is_empty) {
            true => ICON_NOTIFICATIONS.to_owned(),
            false => ICON_NOTIFICATIONS_ACTIVE.to_owned(),
        }
    }));
    let label = create_memo(clone!(notifications -> move || {
        match notifications.with(Vec::len) {
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
        >
            {move |handle: PopoverHandle| view! {
                <NotificationList editor={editor.clone()} popover={handle} />
            }}
        </BarPopup>
    }
}

#[component]
fn NotificationList(editor: Editor, popover: PopoverHandle) -> NodeId {
    let notifications = editor.notifications();
    let empty = create_memo(clone!(notifications -> move || notifications.with(Vec::is_empty)));
    let unclearable = empty.clone();
    let some = create_memo(clone!(empty -> move || !empty.get()));
    let clearing = editor.clone();
    let cleared = notifications.clone();
    let clear = move || {
        let ids = cleared.with_untracked(|listed| listed.iter().map(|shown| shown.id).collect());
        clearing.dismiss_notifications(ids);
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
                    editor={editor.clone()}
                    notifications={notifications.clone()}
                    popover={popover.clone()}
                />
            </Show>
        </List>
    }
}

#[component]
fn NotificationRows(
    editor: Editor,
    notifications: Memo<Vec<HostNotification>>,
    popover: PopoverHandle,
) -> NodeId {
    view! {
        <Frame max_height=Some(LIST_HEIGHT)>
            <Scroll>
                <ForEach keys={notifications}>
                    {move |notification: HostNotification| view! {
                        <NotificationRow
                            editor={editor.clone()}
                            notification
                            popover={popover.clone()}
                        />
                    }}
                </ForEach>
            </Scroll>
        </Frame>
    }
}

#[component]
fn NotificationRow(editor: Editor, notification: HostNotification, popover: PopoverHandle) -> NodeId {
    let theme = use_theme();
    let id = notification.id;
    let activates = notification.has_default_action();
    let dismissing = editor.clone();
    let activate = move || {
        match activates {
            true => {
                editor.invoke_notification(id, HostNotification::DEFAULT_ACTION.to_owned());
                popover.close.call(());
            }
            false => editor.dismiss_notifications(vec![id]),
        }
    };
    let source = match notification.app_name.is_empty() {
        true => received_at(notification.received),
        false => format!(
            "{}  {}",
            notification.app_name,
            received_at(notification.received)
        ),
    };
    let said = !notification.body.is_empty();
    let body = notification.body.clone();
    let summary = notification.summary.clone();
    let body_color = theme.text_muted.clone();
    let summary_color = match notification.critical {
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
                on_click={move || dismissing.dismiss_notifications(vec![id])}
            />
        </List>
    }
}
