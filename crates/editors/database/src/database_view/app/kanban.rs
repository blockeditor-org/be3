use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_CHEVRONS_UP_DOWN, ICON_PLUS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};
use block_editor_beui::database::ChoiceMark;
use database_types::database::{DatabaseValue, short_id};
use database_types::database_schema::{DatabaseField, DatabaseFieldType};
use uuid::Uuid;

use crate::database_view::app::controls::MARK;
use crate::database_view::app::data::Data;
use crate::database_view::app::list::{Parts, group_test_key, row_parts, row_title};
use crate::database_view::groups::{Group, GroupKey};

const COLUMN_WIDTH: f32 = 272.0;
const FOLDED_WIDTH: Size = Size::Xl2;
const COLUMN_SPACING: Space = Space::Lg;
const CARD_SPACING: Space = Space::Md;
const PADDING: Space = Space::Lg;
const DRAG_THRESHOLD: f32 = 6.0;
const CARD_HEIGHT: f32 = 104.0;

#[component]
pub fn Kanban(data: Data) -> NodeId {
    let status = status_field(data);
    let missing = create_memo(move || status.get().is_none());
    let ready = create_memo(move || status.get().is_some());
    let columns = create_memo(move || {
        data.groups.with(|groups| {
            groups
                .iter()
                .map(|group| group.key)
                .collect::<Vec<GroupKey>>()
        })
    });
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show condition={missing}>
                    <Frame padding=PADDING>
                        <Text style=TextStyle::CAPTION>
                            "Choose the choice field whose options make the columns, under Display."
                        </Text>
                    </Frame>
                </Show>
                <Show condition={ready}>
                    <Scroll @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal>
                        <Frame padding=PADDING>
                            <List direction=Direction::Horizontal spacing=COLUMN_SPACING>
                                <ForEach keys={columns}>|key: GroupKey|
                                    <Column data={data} key={key} />
                                </ForEach>
                            </List>
                        </Frame>
                    </Scroll>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Column(data: Data, key: GroupKey) -> NodeId {
    let group = create_memo(move || {
        data.groups
            .with(|groups| groups.iter().find(|group| group.key == key).cloned())
    });
    let folded = create_memo(move || {
        let closed = group.with(|group| group.as_ref().is_some_and(|group| group.closed));
        closed != data.toggled.with(|toggled| toggled.contains(&key))
    });
    let open = create_memo(move || !folded.get());
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Show condition={open}>
                <OpenColumn data={data} group={group} key={key} />
            </Show>
            <Show condition={folded}>
                <FoldedColumn data={data} group={group} key={key} />
            </Show>
        </List>
    }
}

#[component]
fn FoldedColumn(data: Data, group: Memo<Option<Group>>, key: GroupKey) -> NodeId {
    let option = create_memo(move || {
        group.with(|group| group.as_ref().and_then(|group| group.option.clone()))
    });
    let count = create_memo(move || {
        group
            .with(|group| group.as_ref().map_or(0, |group| group.rows.len()))
            .to_string()
    });
    let name = create_memo(move || {
        group.with(|group| {
            group
                .as_ref()
                .map(|group| format!("Show {}", group.label))
                .unwrap_or_default()
        })
    });
    let unfold = move || data.toggle_group(key);
    view! {
        <Frame
            width=FOLDED_WIDTH
            fill=ColorRole::Surface
            radius=Radius::Lg
            padding={Padding::vertical(Space::Md)}
        >
            <List align=Align::Center spacing=Space::Md>
                <ChoiceMark option={option} size=MARK />
                <Text style=TextStyle::CAPTION>{count}</Text>
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRONS_UP_DOWN
                    label={name}
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
                    @test_id={format!("database-view.kanban.unfold.{}", group_test_key(key))}
                    on_click={unfold}
                />
            </List>
        </Frame>
    }
}

#[component]
fn OpenColumn(data: Data, group: Memo<Option<Group>>, key: GroupKey) -> NodeId {
    let label = create_memo(move || {
        group.with(|group| {
            group
                .as_ref()
                .map(|group| group.label.clone())
                .unwrap_or_default()
        })
    });
    let count = create_memo(move || {
        group
            .with(|group| group.as_ref().map_or(0, |group| group.rows.len()))
            .to_string()
    });
    let option = create_memo(move || {
        group.with(|group| group.as_ref().and_then(|group| group.option.clone()))
    });
    let keys = create_memo(move || {
        group.with(|group| {
            group
                .as_ref()
                .map(|group| group.rows.clone())
                .unwrap_or_default()
        })
    });
    let value = create_memo(move || group.with(|group| group.as_ref().and_then(Group::value)));
    let add = move || {
        let Some(field_id) = data.group_by.get_untracked() else {
            return;
        };
        let mut values = Vec::new();
        if let Some(value) = value.get_untracked() {
            values.push((field_id, value));
        }
        if let Some(title) = data.title_field.get_untracked() {
            values.push((title, DatabaseValue::String(String::new())));
        }
        data.new_row(values);
    };
    let read_only = data.read_only;
    let test_key = match key {
        GroupKey::Option(option) => option.to_string(),
        _ => "none".to_owned(),
    };
    let dropped = move |(row, _): (usize, DragPoint)| {
        let Some(field_id) = data.group_by.get_untracked() else {
            return;
        };
        data.set_cell(row, field_id, value.get_untracked());
    };
    view! {
        <DropTarget on_drop={dropped}>|handle: DropHandle| {
            let DropHandle { over, .. } = handle;
            view! {
                <Frame
                    width={Size::Custom(COLUMN_WIDTH)}
                    fill=ColorRole::Surface
                    border={create_memo(move || over.get().then_some(Border::new(ColorRole::Accent).width(Stroke::Focus)))}
                    radius=Radius::Lg
                    padding={Padding::horizontal(Space::Md).bottom(Space::Md)}
                >
                    <List spacing=CARD_SPACING>
                        <Frame padding={Padding::vertical(Space::Xs).left(Space::Xs)}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                                <ChoiceMark option={option} size=MARK />
                                <Text>{label}</Text>
                                <Text style=TextStyle::CAPTION>{count}</Text>
                                <Frame @sizing=ItemSize::Percent(100.0) />
                                <Button
                                    icon_only=true
                                    glyph=ICON_PLUS
                                    label="Add card"
                                    variant=ButtonVariant::Ghost
                                    size=ButtonSize::Small
                                    disabled={read_only}
                                    @test_id={format!("database-view.kanban.add.{test_key}")}
                                    on_click={add}
                                />
                            </List>
                        </Frame>
                        <Scroll @sizing=ItemSize::Percent(100.0)>
                            <VirtualList
                                keys={keys}
                                item_size=CARD_HEIGHT
                                overscan=CARD_HEIGHT
                            >|row: ReadSignal<usize>|
                                <Frame padding={Padding::new(Space::None, Space::None, Space::None, CARD_SPACING)}>
                                    <KanbanCard data={data} row={row} />
                                </Frame>
                            </VirtualList>
                        </Scroll>
                    </List>
                </Frame>
            }
        }</DropTarget>
    }
}

#[component]
fn KanbanCard(data: Data, row: ReadSignal<usize>) -> NodeId {
    let selected = create_memo(move || {
        data.selected
            .get()
            .is_some_and(|selection| selection.row == row.get())
    });
    let pick = move || data.select(row.get_untracked(), None);
    let read_only = data.read_only;
    let payload = create_memo(move || (!read_only.get()).then(|| row.get()));
    let element_test_id = create_memo(move || format!("database-view.card.{}", row.get()));
    let ghost = move |row: usize| {
        view! {
            <Frame width={Size::Custom(COLUMN_WIDTH - 16.0)} @test_id="database-view.card-ghost">
                <CardFace data={data} row={row} />
            </Frame>
        }
    };
    view! {
        <Draggable payload={payload} threshold=DRAG_THRESHOLD preview={ghost}>|_: DragHandle|
            <ListRow
                selected={selected}
                @test_id={element_test_id}
                on_click={pick}
                on_activate={pick}
            >
                <CardLines data={data} row={row} />
            </ListRow>
        </Draggable>
    }
}

#[component]
fn CardFace(data: Data, row: usize) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius={Radius::Tab}
            padding={Padding::symmetric(Space::Md, Space::Md)}
        >
            <CardLines data={data} row={row} />
        </Frame>
    }
}

#[component]
fn CardLines(data: Data, row: Prop<usize>) -> NodeId {
    let entry = create_memo(move || data.rows.with(|rows| rows.get(row.get()).cloned()));
    let code = create_memo(move || {
        data.row_ids
            .with(|ids| ids.get(row.get()).copied())
            .map_or_else(String::new, |id: ObjectId| {
                short_id(&data.prefix.get(), id.as_uuid())
            })
    });
    let title = create_memo(move || {
        let field = data.title_field.get();
        entry.with(|entry| row_title(entry.as_ref(), field))
    });
    let parts = create_memo(move || {
        let title = data.title_field.get();
        let column = data.group_by.get();
        entry.with(|entry| {
            let Some(entry) = entry else {
                return Vec::new();
            };
            data.fields.with(|fields| {
                data.hidden_fields.with(|hidden| {
                    let mut hidden = hidden.clone();
                    hidden.extend(column);
                    data.labels
                        .with(|labels| row_parts(entry, fields, &hidden, labels, title))
                })
            })
        })
    });
    let keys = create_memo(move || {
        parts.with(|parts| parts.iter().map(|(key, _)| *key).collect::<Vec<Uuid>>())
    });
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::CAPTION>{code}</Text>
            <Text>{title}</Text>
            <Parts parts={parts} keys={keys} wrap=true />
        </List>
    }
}

pub fn status_field(data: Data) -> Memo<Option<DatabaseField>> {
    let fields = data.fields;
    let chosen = data.group_by;
    create_memo(move || {
        let chosen = chosen.get()?;
        fields.with(|fields| {
            fields
                .iter()
                .find(|field| field.id == chosen && field.field_type == DatabaseFieldType::Enum)
                .cloned()
        })
    })
}
