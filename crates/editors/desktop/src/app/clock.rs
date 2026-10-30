use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::popover::PANEL_PADDING;
use block_editor_beui::beui::styled::*;

use super::calendar::{CALENDAR_WIDTH, DesktopCalendar, SCREEN_SHARE};
use super::notifications::{DesktopNotifications, LIST_HEIGHT, NotificationList};
use super::popup::BarPopup;
use beui::{Align, Direction};

const SPACING: Space = Space::Xl;
const COUNT_SPACING: Space = Space::Sm;
const COUNT_PADDING: Padding = Padding::symmetric(Space::Sm, Space::None);
const HISTORY: Size = Size::Popover;
const BESIDE_WIDTH: f32 = CALENDAR_WIDTH + HISTORY.px() + SPACING.px() + 2.0 * PANEL_PADDING.px();
const STACKED_CALENDAR_SHARE: f32 = 0.45;
const STACKED_HISTORY_SHARE: f32 = 0.2;
const BESIDE_HISTORY_SHARE: f32 = 0.6;

#[component]
pub(crate) fn ClockButton(
    editor: Editor,
    notifications: DesktopNotifications,
    clock: ReadSignal<String>,
) -> NodeId {
    let listed = notifications.listed();
    let offered = notifications.offered;
    let count = create_memo(move || match offered.get() {
        true => listed.with(Vec::len),
        false => 0,
    });
    let unread = create_memo(move || count.get() > 0);
    let counted = create_memo(move || count.get().to_string());
    let label = create_memo(move || match count.get() {
        0 => clock.get(),
        1 => format!("{}, 1 notification", clock.get()),
        count => format!("{}, {count} notifications", clock.get()),
    });
    let screen = container_size();
    let room = create_memo(move || screen.as_ref().map(|screen| screen.get()));
    let beside = create_memo(move || {
        room.get()
            .is_none_or(|room| room.x <= 0.0 || room.x >= BESIDE_WIDTH + 2.0 * SPACING.px())
    });
    let width = create_memo(move || match offered.get() && beside.get() {
        true => Size::Custom(BESIDE_WIDTH),
        false => Size::Custom(CALENDAR_WIDTH),
    });
    view! {
        <BarPopup
            label
            icon_only=true
            width
            @test_id={"desktop.clock"}
            open={notifications.browsing()}
            leading={view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=COUNT_SPACING>
                    <Text>{clock}</Text>
                    <Show condition={unread}>
                        <Frame
                            fill=ColorRole::Accent
                            radius=Radius::Full
                            padding=COUNT_PADDING
                            @test_id={"desktop.clock.unread"}
                        >
                            <Text size=TextSize::Xs weight=Weight::Bold color=TextColor::OnAccent>
                                {counted}
                            </Text>
                        </Frame>
                    </Show>
                </List>
            }}
        >|handle: PopoverHandle|
            <ClockPanel
                editor={editor}
                notifications={notifications}
                beside={beside}
                room={room}
                popover={handle}
            />
        </BarPopup>
    }
}

#[component]
fn ClockPanel(
    editor: Editor,
    notifications: DesktopNotifications,
    beside: Memo<bool>,
    room: Memo<Option<beui::Vec2>>,
    popover: PopoverHandle,
) -> NodeId {
    let offered = notifications.offered;
    let listed = notifications.listed();
    let stacked = create_memo(move || offered.get() && !beside.get());
    let direction = create_memo(move || match beside.get() {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    });
    let share = create_memo(move || match stacked.get() {
        true => STACKED_CALENDAR_SHARE,
        false => SCREEN_SHARE,
    });
    let height = create_memo(move || {
        let tall = room.get().map_or(0.0, |room| room.y);
        let share = match stacked.get() {
            true => STACKED_HISTORY_SHARE,
            false => BESIDE_HISTORY_SHARE,
        };
        match tall > 0.0 {
            true => LIST_HEIGHT.min(tall * share),
            false => LIST_HEIGHT,
        }
    });
    let history_width = create_memo(move || match beside.get() {
        true => Length::Size(HISTORY),
        false => Length::FILL,
    });
    view! {
        <List direction={direction} align=Align::Start spacing=SPACING>
            <DesktopCalendar editor={editor} share={share} />
            <Show condition={offered}>
                <Frame width={history_width}>
                    <NotificationList
                        notifications={notifications}
                        listed={listed}
                        height={height}
                        popover={popover}
                    />
                </Frame>
            </Show>
        </List>
    }
}
