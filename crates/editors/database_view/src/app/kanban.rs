use block_client::blocks::database::DatabaseValue;
use block_client::blocks::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_plugin::beui::icons::ICON_ADD;
use block_editor_plugin::beui::reactive::{
    Direction, ForEach, Frame, ItemSize, List, Memo, Scroll, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, ListRow, Separator, use_theme,
};
use block_editor_plugin::beui::{NodeId, TextAlign};
use block_editor_plugin::block_ui::database::cell_text;
use uuid::Uuid;

use crate::app::data::Data;

const COLUMN_WIDTH: f32 = 240.0;
const COLUMN_SPACING: f32 = 10.0;
const CARD_SPACING: f32 = 6.0;
const PADDING: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColumnKey(pub Option<Uuid>);

#[component]
pub fn Kanban(data: Data) -> NodeId {
    let status = status_field(&data);
    let missing = create_memo(clone!(status -> move || status.get().is_none()));
    let ready = create_memo(clone!(status -> move || status.get().is_some()));
    let columns = create_memo(clone!(status -> move || {
        let mut columns: Vec<ColumnKey> = status
            .get()
            .map(|field| {
                field
                    .enum_options
                    .iter()
                    .map(|option| ColumnKey(Some(option.id)))
                    .collect()
            })
            .unwrap_or_default();
        columns.push(ColumnKey(None));
        columns
    }));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={missing}>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <Caption content="Choose a status field to use the kanban view." />
                    </Frame>
                </Show>
                <Show condition={ready}>
                    <Scroll
                        @sizing=ItemSize::Percent(100.0)
                        direction=Direction::Horizontal
                        focus_color={theme.accent.clone()}
                    >
                        <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                            <List direction=Direction::Horizontal spacing=COLUMN_SPACING>
                                <ForEach keys={columns}>
                                    {move |column: ColumnKey| {
                                        let data = data.clone();
                                        let status = status.clone();
                                        view! {
                                            <Column data={data} status={status} column={column} />
                                        }
                                    }}
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
fn Column(data: Data, status: Memo<Option<DatabaseField>>, column: ColumnKey) -> NodeId {
    let option = column.0;
    let title = create_memo(clone!(status -> move || {
        let Some(option) = option else {
            return "No status".to_owned();
        };
        status.with(|status| {
            status
                .as_ref()
                .and_then(|status| status.enum_options.iter().find(|shown| shown.id == option))
                .map_or_else(|| "Unknown".to_owned(), |shown| shown.name.clone())
        })
    }));
    let rows = data.rows.clone();
    let field_id = create_memo(clone!(status -> move || {
        status.with(|status| status.as_ref().map(|status| status.id))
    }));
    let keys = create_memo(clone!(rows field_id -> move || {
        let Some(field_id) = field_id.get() else {
            return Vec::new();
        };
        rows.with(|rows| {
            rows.iter()
                .enumerate()
                .filter(|(_, row)| match row.value(field_id) {
                    Some(DatabaseValue::Enum(id)) => option == Some(*id),
                    _ => option.is_none(),
                })
                .map(|(index, _)| index)
                .collect::<Vec<usize>>()
        })
    }));
    let add = clone!(data field_id -> move || {
        let Some(field_id) = field_id.get_untracked() else {
            return;
        };
        let row = data.rows.with_untracked(Vec::len);
        data.select(row, None);
        data.set_cell(row, field_id, option.map(DatabaseValue::Enum));
    });
    let read_only = data.read_only.clone();
    let cards = data.clone();
    let key = option.map_or_else(|| "none".to_owned(), |option| option.to_string());
    view! {
        <Frame width=COLUMN_WIDTH>
            <Card>
                <List spacing=CARD_SPACING>
                    <Body content={title} />
                    <Separator />
                    <ForEach keys={keys}>
                        {move |row: usize| {
                            let data = cards.clone();
                            let status = field_id.clone();
                            view! {
                                <KanbanCard data={data} status={status} row={row} />
                            }
                        }}
                    </ForEach>
                    <List direction=Direction::Horizontal spacing=0.0>
                        <Button
                            glyph={ICON_ADD.to_owned()}
                            label="Add card"
                            variant=ButtonVariant::Secondary
                            disabled={read_only}
                            @test_id={format!("database-view.kanban.add.{key}")}
                            on_click={add}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                </List>
            </Card>
        </Frame>
    }
}

#[component]
fn KanbanCard(data: Data, status: Memo<Option<Uuid>>, row: usize) -> NodeId {
    let lines = create_memo(clone!(data status -> move || {
        let status = status.get();
        let Some(entry) = data.rows.with(|rows| rows.get(row).cloned()) else {
            return Vec::new();
        };
        let shown = data.fields.with(|fields| {
            data.labels.with(|labels| {
                fields
                    .iter()
                    .filter(|field| Some(field.id) != status)
                    .map(|field| cell_text(&entry, field, labels))
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<String>>()
            })
        });
        match shown.is_empty() {
            true => vec![format!("Row {}", row + 1)],
            false => shown,
        }
    }));
    let keys =
        create_memo(clone!(lines -> move || (0..lines.with(Vec::len)).collect::<Vec<usize>>()));
    let selected = create_memo(clone!(data -> move || {
        data.selected.get().is_some_and(|selection| selection.row == row)
    }));
    let pick = clone!(data -> move || data.select(row, None));
    view! {
        <ListRow
            selected={selected}
            @test_id={format!("database-view.card.{row}")}
            on_click={pick.clone()}
            on_activate={pick}
        >
            <List spacing=2.0>
                <ForEach keys={keys}>
                    {move |line: usize| {
                        let text = create_memo(clone!(lines -> move || {
                            lines.with(|lines| lines.get(line).cloned().unwrap_or_default())
                        }));
                        view! {
                            <Caption content={text} align=TextAlign::Start wrap=true />
                        }
                    }}
                </ForEach>
            </List>
        </ListRow>
    }
}

pub fn status_field(data: &Data) -> Memo<Option<DatabaseField>> {
    let fields = data.fields.clone();
    let chosen = data.kanban_field.clone();
    create_memo(move || {
        let chosen = chosen.get()?;
        fields.with(|fields| fields.iter().find(|field| field.id == chosen).cloned())
    })
}

pub fn enum_fields(data: &Data) -> Memo<Vec<DatabaseField>> {
    let fields = data.fields.clone();
    create_memo(move || {
        fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type == DatabaseFieldType::Enum)
                .cloned()
                .collect()
        })
    })
}
