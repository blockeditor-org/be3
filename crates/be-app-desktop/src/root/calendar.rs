use std::collections::BTreeMap;

use be_sdk_beui::be_block::Root;
use be_sdk_beui::be_block::ViewState;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::datetime::{Date, HourCycle, SECONDS_PER_DAY, Time};
use be_sdk_beui::beui::icons::ICON_CALENDAR_DAYS;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::calendar::CALENDAR_WIDTH as GRID_WIDTH;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockParent, BlockQuery, Editor, utc_offset};
use be_sdk_shell::Workspace;
use be_type_database::database::{Database, DatabaseContent, DatabaseValue};
use be_type_database::database_schema::{DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent};
use be_type_database::database_view::{DatabaseView, DatabaseViewContent, DatabaseViewKind};
use beui::{Align, Direction, Extent, ItemSize};
use uuid::Uuid;

pub(crate) const CALENDAR_STATE: &str = "desktop.calendar";
pub(crate) const CALENDAR_WIDTH: f32 = GRID_WIDTH;
const SPACING: Space = Space::Md;
const EVENT_SPACING: Space = Space::Xs2;
const UNTITLED: &str = "Untitled";
const TIME_WIDTH: Size = Size::Xl;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DayEvent {
    starts: i64,
    title: String,
}

fn today() -> Date {
    let unix = i64::try_from(be_sdk_beui::wall_clock().as_secs()).unwrap_or(0);
    Date::from_days((unix + i64::from(utc_offset())).div_euclid(SECONDS_PER_DAY))
}

#[component]
pub(crate) fn DesktopCalendar(workspace: Workspace, popover: PopoverHandle) -> NodeId {
    let editor = workspace.editor();
    let block = calendar_block(editor);
    let today = today();
    let picked = create_signal(Some(today));
    let day = create_memo(move || picked.get().unwrap_or(today));
    let heading = create_memo(move || match day.get() == today {
        true => "Today".to_owned(),
        false => {
            let day = day.get();
            format!("{} {} {}", day.weekday().name(), day.day, day.month_name())
        }
    });
    let events = day_events(editor, block, day);
    let none = create_memo(move || events.with(Vec::is_empty));
    let some = create_memo(move || !none.get());
    let unopened = create_memo(move || block.get().is_none());
    let open = move || {
        if let Some(block) = block.get_untracked() {
            workspace.open_block(block, DatabaseView::CONTENT_TYPE);
            popover.close.call(());
        }
    };
    view! {
        <Frame width={Extent::at_most(CALENDAR_WIDTH)}>
            <List spacing=SPACING @test_id={"desktop.calendar"}>
                <Calendar selected={picked} today={Some(today)} width={Length::FILL} />
                <List spacing=EVENT_SPACING @test_id={"desktop.calendar.events"}>
                    <Text weight=Weight::Bold>{heading}</Text>
                    <Show condition={none}>
                        <Text style=TextStyle::CAPTION>"No events"</Text>
                    </Show>
                    <Show condition={some}>
                        <ForEach keys={events}>|event: DayEvent|
                            <EventRow event />
                        </ForEach>
                    </Show>
                </List>
                <Button
                    label="Open calendar"
                    glyph=ICON_CALENDAR_DAYS
                    variant=ButtonVariant::Ghost
                    disabled={unopened}
                    @test_id={"desktop.calendar.open"}
                    on_click={open}
                />
            </List>
        </Frame>
    }
}

#[component]
fn EventRow(event: DayEvent) -> NodeId {
    let minutes = event.starts.rem_euclid(SECONDS_PER_DAY) / 60;
    let at = Time::from_minutes(u32::try_from(minutes).unwrap_or(0)).format(HourCycle::H24);
    view! {
        <List direction=Direction::Horizontal align=Align::Baseline spacing=SPACING>
            <Frame width=TIME_WIDTH>
                <Text style=TextStyle::CAPTION>{at}</Text>
            </Frame>
            <Text @sizing={ItemSize::Intrinsic.shrink(1.0)}>{event.title}</Text>
        </List>
    }
}

fn day_events(editor: Editor, block: Memo<Option<Uuid>>, day: Memo<Date>) -> Memo<Vec<DayEvent>> {
    let view = editor.related_content::<DatabaseViewContent>(block);
    let found = view.project(|view| {
        let view = view.root();
        (view.database, view.calendar_start)
    });
    let database_id = create_memo(move || found.get().0);
    let start = create_memo(move || found.get().1);
    let database = editor.related_content::<DatabaseContent>(database_id);
    let schema = database.project(|database| database.root().schema);
    let schema_id = create_memo(move || schema.get());
    let schema = editor.related_content::<DatabaseSchemaContent>(schema_id);
    let title = schema.project(|schema| schema.root().title_field());
    let rows = database.project(|database| {
        database
            .root()
            .rows
            .iter()
            .map(|row| row.values().clone())
            .collect::<Vec<BTreeMap<Uuid, DatabaseValue>>>()
    });
    create_memo(move || {
        let Some(start) = start.get() else {
            return Vec::new();
        };
        let title = title.get();
        let first = day.get().days() * SECONDS_PER_DAY;
        let day = first..first + SECONDS_PER_DAY;
        let mut events: Vec<DayEvent> = rows.with(|rows| {
            rows.iter()
                .filter_map(|values| {
                    let Some(DatabaseValue::Datetime(starts)) = values.get(&start) else {
                        return None;
                    };
                    let title = title
                        .and_then(|title| match values.get(&title) {
                            Some(DatabaseValue::String(text)) if !text.trim().is_empty() => {
                                Some(text.clone())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| UNTITLED.to_owned());
                    day.contains(starts).then(|| DayEvent {
                        starts: *starts,
                        title,
                    })
                })
                .collect()
        });
        events.sort_by(|left, right| {
            left.starts
                .cmp(&right.starts)
                .then_with(|| left.title.cmp(&right.title))
        });
        events.dedup();
        events
    })
}

fn calendar_block(editor: Editor) -> Memo<Option<Uuid>> {
    let view = editor.view_content();
    let stored = create_memo(move || {
        view.as_ref().and_then(|view| {
            view.read(|held| {
                held.root()
                    .state(CALENDAR_STATE)
                    .and_then(|state| state.refs.first().copied())
                    .filter(|id| !id.is_nil())
            })
            .ok()
            .flatten()
        })
    });
    let children = editor.watch_blocks(BlockQuery::Children(editor.block_id()));
    let created = create_signal(None::<Uuid>);
    let held = create_memo(move || {
        stored.get().filter(|id| {
            created.get() == Some(*id)
                || children.with(|children| {
                    children.as_ref().map_or(true, |children| {
                        children.iter().any(|child| child.id == *id)
                    })
                })
        })
    });
    let editable = editor.editable();
    create_effect(move || {
        let ready = view.as_ref().is_some_and(|view| view.ready().is_ok());
        if !ready || !editable.get() || held.get().is_some() {
            return;
        }
        untrack(|| {
            let calendar = create_calendar(editor);
            created.set(Some(calendar));
            editor.set_view_state(CALENDAR_STATE, Some(&ViewState::new(&(), vec![calendar])));
        });
    });
    held
}

fn create_calendar(editor: Editor) -> Uuid {
    let blocks = editor.blocks();
    let mut schema = DatabaseSchemaContent::default();
    let mut add = |name: &str, field_type: DatabaseFieldType| {
        let (id, edit) = DatabaseSchema::add_field(name, field_type);
        schema.apply(&edit);
        id
    };
    add("Name", DatabaseFieldType::String);
    let start = add("Starts", DatabaseFieldType::Datetime);
    let end = add("Ends", DatabaseFieldType::Datetime);
    let schema = blocks.create(&schema, BlockParent::Detached);
    let database = blocks.create_named(
        &DatabaseContent::new(&Database::with_schema(schema)),
        BlockParent::Detached,
        "Calendar",
    );
    blocks.set_parent(schema, BlockParent::Block(database));
    let view = editor.create_child(&DatabaseViewContent::new(&DatabaseView {
        kind: DatabaseViewKind::Calendar,
        calendar_start: Some(start),
        calendar_end: Some(end),
        ..DatabaseView::of(database)
    }));
    blocks.set_parent(database, BlockParent::Block(view));
    view
}
