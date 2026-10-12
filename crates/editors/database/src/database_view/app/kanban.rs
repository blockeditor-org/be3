use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{DatabaseValue, short_id};
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ADD, ICON_UNFOLD_MORE};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component, create_memo,
    view,
};
use block_editor_beui::beui::styled::{
    Body, ButtonVariant, Caption, IconButton, IconButtonSize, ListRow, Scroll, use_theme,
};
use block_editor_beui::beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};
use block_editor_beui::database::ChoiceMark;
use uuid::Uuid;

use crate::database_view::app::controls::MARK_SIZE;
use crate::database_view::app::data::Data;
use crate::database_view::app::list::{Parts, group_test_key, row_parts, row_title};
use crate::database_view::groups::{Group, GroupKey};

const COLUMN_WIDTH: f32 = 272.0;
const FOLDED_WIDTH: f32 = 48.0;
const COLUMN_SPACING: f32 = 12.0;
const CARD_SPACING: f32 = 8.0;
const PADDING: f32 = 12.0;
const DRAG_THRESHOLD: f32 = 6.0;

#[component]
pub fn Kanban(data: Data) -> NodeId {
    let status = status_field(&data);
    let missing = create_memo(clone!(status -> move || status.get().is_none()));
    let ready = create_memo(clone!(status -> move || status.get().is_some()));
    let columns = create_memo(clone!(data -> move || {
        data.groups.with(|groups| groups.iter().map(|group| group.key).collect::<Vec<GroupKey>>())
    }));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={missing}>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <Caption
                            content="Choose the choice field whose options make the columns, under Display."
                            wrap=true
                        />
                    </Frame>
                </Show>
                <Show condition={ready}>
                    {move || clone!(columns data -> view! {
                        <Scroll @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal>
                            <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                                <List direction=Direction::Horizontal spacing=COLUMN_SPACING>
                                    <ForEach keys={columns}>
                                        {move |key: GroupKey| {
                                            let data = data.clone();
                                            view! {
                                                <Column data={data} key={key} />
                                            }
                                        }}
                                    </ForEach>
                                </List>
                            </Frame>
                        </Scroll>
                    })}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Column(data: Data, key: GroupKey) -> NodeId {
    let group = create_memo(clone!(data -> move || {
        data.groups.with(|groups| groups.iter().find(|group| group.key == key).cloned())
    }));
    let folded = create_memo(clone!(data group -> move || {
        let closed = group.with(|group| group.as_ref().is_some_and(|group| group.closed));
        closed != data.toggled.with(|toggled| toggled.contains(&key))
    }));
    let open = create_memo(clone!(folded -> move || !folded.get()));
    let (open_data, folded_data) = (data.clone(), data);
    let (open_group, folded_group) = (group.clone(), group);
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Show condition={open}>
                {move || clone!(open_data open_group -> view! {
                    <OpenColumn data={open_data} group={open_group} key={key} />
                })}
            </Show>
            <Show condition={folded}>
                {move || clone!(folded_data folded_group -> view! {
                    <FoldedColumn data={folded_data} group={folded_group} key={key} />
                })}
            </Show>
        </List>
    }
}

#[component]
fn FoldedColumn(data: Data, group: Memo<Option<Group>>, key: GroupKey) -> NodeId {
    let option = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().and_then(|group| group.option.clone()))
    }));
    let count = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().map_or(0, |group| group.rows.len())).to_string()
    }));
    let name = create_memo(clone!(group -> move || {
        group.with(|group| {
            group
                .as_ref()
                .map(|group| format!("Show {}", group.label))
                .unwrap_or_default()
        })
    }));
    let unfold = clone!(data -> move || data.toggle_group(key));
    let theme = use_theme();
    view! {
        <Frame width=FOLDED_WIDTH color={theme.surface.clone()} radius=10 padding_vertical=10.0>
            <List align=Align::Center spacing=8.0>
                <ChoiceMark option={option} size=MARK_SIZE />
                <Caption content={count} />
                <IconButton
                    glyph={ICON_UNFOLD_MORE.to_owned()}
                    label={name}
                    variant=ButtonVariant::Ghost
                    size=IconButtonSize::Compact
                    @test_id={format!("database-view.kanban.unfold.{}", group_test_key(key))}
                    on_click={unfold}
                />
            </List>
        </Frame>
    }
}

#[component]
fn OpenColumn(data: Data, group: Memo<Option<Group>>, key: GroupKey) -> NodeId {
    let label = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().map(|group| group.label.clone()).unwrap_or_default())
    }));
    let count = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().map_or(0, |group| group.rows.len())).to_string()
    }));
    let option = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().and_then(|group| group.option.clone()))
    }));
    let keys = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().map(|group| group.rows.clone()).unwrap_or_default())
    }));
    let value = create_memo(clone!(group -> move || {
        group.with(|group| group.as_ref().and_then(Group::value))
    }));
    let add = clone!(data value -> move || {
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
    });
    let read_only = data.read_only.clone();
    let cards = data.clone();
    let test_key = match key {
        GroupKey::Option(option) => option.to_string(),
        _ => "none".to_owned(),
    };
    let dropped = clone!(data value -> move |(row, _): (usize, DragPoint)| {
        let Some(field_id) = data.group_by.get_untracked() else {
            return;
        };
        data.set_cell(row, field_id, value.get_untracked());
    });
    let theme = use_theme();
    view! {
        <DropTarget on_drop={dropped}>
            {move |handle: DropHandle| {
                let DropHandle { over, .. } = handle;
                let cards = cards.clone();
                view! {
                    <Frame
                        width=COLUMN_WIDTH
                        color={theme.surface.clone()}
                        outline={theme.accent.clone()}
                        outline_width=2.0
                        radius=10
                        outline_visible={over}
                        padding_horizontal=8.0
                        padding_bottom={Some(8.0)}
                    >
                        <List spacing=CARD_SPACING>
                            <Frame padding_left={Some(4.0)} padding_vertical=4.0>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=8.0
                                >
                                    <ChoiceMark option={option.clone()} size=MARK_SIZE />
                                    <Body content={label.clone()} />
                                    <Caption content={count.clone()} />
                                    <Frame @sizing=ItemSize::Percent(100.0) />
                                    <IconButton
                                        glyph={ICON_ADD.to_owned()}
                                        label="Add card"
                                        variant=ButtonVariant::Ghost
                                        size=IconButtonSize::Compact
                                        disabled={read_only.clone()}
                                        @test_id={format!("database-view.kanban.add.{test_key}")}
                                        on_click={add.clone()}
                                    />
                                </List>
                            </Frame>
                            <ForEach keys={keys.clone()}>
                                {move |row: usize| {
                                    let data = cards.clone();
                                    view! {
                                        <KanbanCard data={data} row={row} />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                }
            }}
        </DropTarget>
    }
}

#[component]
fn KanbanCard(data: Data, row: usize) -> NodeId {
    let selected = create_memo(clone!(data -> move || {
        data.selected.get().is_some_and(|selection| selection.row == row)
    }));
    let pick = clone!(data -> move || data.select(row, None));
    let read_only = data.read_only.clone();
    let payload = create_memo(move || (!read_only.get()).then_some(row));
    let ghost = clone!(data -> move |row: usize| {
        let data = data.clone();
        view! {
            <Frame width={COLUMN_WIDTH - 16.0} @test_id="database-view.card-ghost">
                <CardFace data={data} row={row} />
            </Frame>
        }
    });
    view! {
        <Draggable payload={payload} threshold=DRAG_THRESHOLD preview={ghost}>
            {move |_: DragHandle| view! {
                <ListRow
                    selected={selected.clone()}
                    @test_id={format!("database-view.card.{row}")}
                    on_click={pick.clone()}
                    on_activate={pick.clone()}
                >
                    <CardLines data={data.clone()} row={row} />
                </ListRow>
            }}
        </Draggable>
    }
}

#[component]
fn CardFace(data: Data, row: usize) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=8
            padding_horizontal=10.0
            padding_vertical=8.0
        >
            <CardLines data={data} row={row} />
        </Frame>
    }
}

#[component]
fn CardLines(data: Data, row: usize) -> NodeId {
    let entry = create_memo(clone!(data -> move || data.rows.with(|rows| rows.get(row).cloned())));
    let code = create_memo(clone!(data -> move || {
        data.row_ids
            .with(|ids| ids.get(row).copied())
            .map_or_else(String::new, |id: ObjectId| short_id(&data.prefix.get(), id.as_uuid()))
    }));
    let title = create_memo(clone!(data entry -> move || {
        let field = data.title_field.get();
        entry.with(|entry| row_title(entry.as_ref(), field))
    }));
    let parts = create_memo(clone!(data entry -> move || {
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
                    data.labels.with(|labels| row_parts(entry, fields, &hidden, labels, title))
                })
            })
        })
    }));
    let keys = create_memo(clone!(parts -> move || {
        parts.with(|parts| parts.iter().map(|(key, _)| *key).collect::<Vec<Uuid>>())
    }));
    view! {
        <List spacing=6.0>
            <Caption content={code} />
            <Body content={title} wrap=true />
            <List direction=Direction::Horizontal spacing=0.0 wrap=true>
                <Parts parts={parts} keys={keys} />
            </List>
        </List>
    }
}

pub fn status_field(data: &Data) -> Memo<Option<DatabaseField>> {
    let fields = data.fields.clone();
    let chosen = data.group_by.clone();
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
