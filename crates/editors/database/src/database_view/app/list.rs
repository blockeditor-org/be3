use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{DatabaseRow, DatabaseValue, short_id};
use block_editor_beui::be_block::database_schema::{
    ChoiceIcon, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
};
use block_editor_beui::beui::icons::{ICON_ADD, ICON_DESCRIPTION, ICON_EXPAND_MORE, ICON_LINK};
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, ForEach, Frame, ItemSize, Layers, List, Memo, Show, VirtualList,
    component, create_memo, view,
};
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::{
    ButtonVariant, IconButton, IconButtonSize, ListRow, Scroll, Separator, TextColor, use_theme,
};
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::block_ui::database::cell_text;
use block_editor_beui::database::{ChoiceMark, choice_color};
use block_editor_beui::narrow_chrome;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::app::row::RowPanel;
use crate::database_view::groups::{Group, GroupKey};
use crate::database_view::sort::BlockLabels;
use block_editor_beui::beui::styled::{Icon, Text, TextSize, TextStyle};

const PANEL_WIDTH: f32 = 460.0;
const LINE_HEIGHT: f32 = 38.0;
const CODE_WIDTH: f32 = 76.0;
const MARK: TextSize = TextSize::Sm;
const MARK_SIZE: f32 = MARK.icon_px();
const SPACING: f32 = 10.0;
const GUTTER: f32 = 12.0;
const PAGE_PART: Uuid = Uuid::nil();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Line {
    Header(GroupKey),
    Row(ObjectId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Choice(DatabaseEnumOption),
    Link(String),
    Links(String, usize),
    Text(String),
    Page,
}

#[component]
pub fn ListView(data: Data) -> NodeId {
    let groups = visible_groups(data);
    let lines = create_memo(move || {
        let toggled = data.toggled.get();
        let ids = data.row_ids.get();
        groups.with(|groups| {
            let mut lines = Vec::new();
            for group in groups {
                if group.key != GroupKey::All {
                    lines.push(Line::Header(group.key));
                }
                if group.closed != toggled.contains(&group.key) {
                    continue;
                }
                lines.extend(
                    group
                        .rows
                        .iter()
                        .filter_map(|index| ids.get(*index).copied().map(Line::Row)),
                );
            }
            lines
        })
    });
    let empty =
        create_memo(move || groups.with(|groups| groups.iter().all(|group| group.rows.is_empty())));
    let blank = create_memo(
        move || match data.search.with(|search| search.trim().is_empty()) {
            true => "Nothing here yet.".to_owned(),
            false => "No rows match the search.".to_owned(),
        },
    );
    let theme = use_theme();
    view! {
        <Frame color={theme.background}>
            <Scroll>
                <Frame padding_bottom={Some(GUTTER * 2.0)}>
                    <List spacing=0.0>
                        <Show condition={empty}>
                            {move || view! {
                                <Frame padding_horizontal={GUTTER * 1.5} padding_vertical=GUTTER>
                                    <Text style=TextStyle::CAPTION string={blank} />
                                </Frame>
                            }}
                        </Show>
                        <VirtualList keys={lines} item_size=LINE_HEIGHT>
                            {move |line: Line| {
                                let groups = groups;
                                match line {
                                    Line::Header(key) => view! {
                                        <GroupHeader data={data} groups={groups} key={key} />
                                    },
                                    Line::Row(row) => view! {
                                        <RowLine data={data} row={row} />
                                    },
                                }
                            }}
                        </VirtualList>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
pub fn RowBeside(
    data: Data,
    enabled: bool,
    children: block_editor_beui::beui::reactive::Child,
) -> NodeId {
    let selected = create_memo(move || {
        match enabled
            && data.kind.get()
                != block_editor_beui::be_block::database_view::DatabaseViewKind::Spreadsheet
        {
            true => data.selected_row(),
            false => None,
        }
    });
    let open = create_memo(move || selected.get().is_some());
    let narrow = narrow_chrome();
    let main_shown = create_memo(move || !(narrow.get() && open.get()));
    let beside = create_memo(move || !narrow.get() && open.get());
    let instead = create_memo(move || narrow.get() && open.get());
    let (side_row, full_row) = (selected, selected);
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Layers @sizing=ItemSize::Percent(100.0)>
                <Frame visible={main_shown}>{children}</Frame>
                <Show condition={instead}>
                    {move || view! {
                        <OpenRow data={data} row={full_row} />
                    }}
                </Show>
            </Layers>
            <Show condition={beside}>
                {move || view! {
                    <Frame @sizing=ItemSize::Fixed(PANEL_WIDTH)>
                        <List direction=Direction::Horizontal spacing=0.0>
                            <Separator direction=Direction::Vertical />
                            <OpenRow @sizing=ItemSize::Percent(100.0) data={data} row={side_row} />
                        </List>
                    </Frame>
                }}
            </Show>
        </List>
    }
}

#[component]
fn OpenRow(data: Data, row: Memo<Option<ObjectId>>) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={row}>
                {move |row: Option<ObjectId>| {
                    match row {
                        Some(row) => view! {
                            <RowPanel @sizing=ItemSize::Percent(100.0) data={data} row={row} />
                        },
                        None => view! {
                            <Frame @sizing=ItemSize::Percent(100.0) />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

pub fn visible_groups(data: Data) -> Memo<Vec<Group>> {
    data.groups
}

#[component]
fn GroupHeader(data: Data, groups: Memo<Vec<Group>>, key: GroupKey) -> NodeId {
    let group = create_memo(move || {
        groups.with(|groups| groups.iter().find(|group| group.key == key).cloned())
    });
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
    let marked = create_memo(move || {
        option.with(|option| {
            option
                .as_ref()
                .is_some_and(|option| option.icon != ChoiceIcon::None)
        })
    });
    let collapsed = create_memo(move || {
        let closed = group.with(|group| group.as_ref().is_some_and(|group| group.closed));
        closed != data.toggled.with(|toggled| toggled.contains(&key))
    });
    let chevron = create_memo(move || match collapsed.get() {
        true => "Expand".to_owned(),
        false => "Collapse".to_owned(),
    });
    let toggle = move || data.toggle_group(key);
    let add = move || {
        let mut values = Vec::new();
        let field = data.group_by.get_untracked();
        if let (Some(field), Some(value)) = (
            field,
            group.with_untracked(|group| group.as_ref().and_then(Group::value)),
        ) {
            values.push((field, value));
        }
        if let Some(title) = data.title_field.get_untracked() {
            values.push((title, DatabaseValue::String(String::new())));
        }
        data.new_row(values);
    };
    let read_only = data.read_only;
    let test_key = group_test_key(key);
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface}
            padding_left={Some(GUTTER * 0.5)}
            padding_right={Some(GUTTER * 0.5)}
            padding_vertical=2.0
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_EXPAND_MORE.to_owned()}
                    label={chevron}
                    variant=ButtonVariant::Ghost
                    size=IconButtonSize::Compact
                    @test_id={format!("database-view.group.{test_key}.toggle")}
                    on_click={toggle}
                />
                <Frame visible={marked}>
                    <ChoiceMark option={option} size=MARK />
                </Frame>
                <Text string={label} />
                <Text style=TextStyle::CAPTION string={count} />
                <Frame @sizing=ItemSize::Percent(100.0) />
                <IconButton
                    glyph={ICON_ADD.to_owned()}
                    label="Add a row to this group"
                    variant=ButtonVariant::Ghost
                    size=IconButtonSize::Compact
                    disabled={read_only}
                    @test_id={format!("database-view.group.{test_key}.add")}
                    on_click={add}
                />
            </List>
        </Frame>
    }
}

pub fn group_test_key(key: GroupKey) -> String {
    match key {
        GroupKey::All => "all".to_owned(),
        GroupKey::Empty => "empty".to_owned(),
        GroupKey::Option(option) => option.to_string(),
        GroupKey::Row(row) => row.row.to_string(),
    }
}

#[component]
fn RowLine(data: Data, row: ObjectId) -> NodeId {
    let index = create_memo(move || {
        data.row_ids
            .with(|ids| ids.iter().position(|id| *id == row))
    });
    let entry = create_memo(move || {
        let index = index.get()?;
        data.rows.with(|rows| rows.get(index).cloned())
    });
    let selected = create_memo(move || {
        let index = index.get();
        data.selected
            .get()
            .is_some_and(|selection| Some(selection.row) == index)
    });
    let code = create_memo(move || short_id(&data.prefix.get(), row.as_uuid()));
    let title = create_memo(move || {
        let field = data.title_field.get();
        entry.with(|entry| row_title(entry.as_ref(), field))
    });
    let marks = create_memo(move || {
        data.fields.with(|fields| {
            data.hidden_fields.with(|hidden| {
                fields
                    .iter()
                    .filter(|field| !hidden.contains(&field.id) && iconic(field))
                    .map(|field| field.id)
                    .collect::<Vec<Uuid>>()
            })
        })
    });
    let parts = create_memo(move || {
        let title = data.title_field.get();
        entry.with(|entry| {
            let Some(entry) = entry else {
                return Vec::new();
            };
            data.fields.with(|fields| {
                data.hidden_fields.with(|hidden| {
                    data.labels
                        .with(|labels| row_parts(entry, fields, hidden, labels, title))
                })
            })
        })
    });
    let part_keys = create_memo(move || {
        parts.with(|parts| parts.iter().map(|(key, _)| *key).collect::<Vec<Uuid>>())
    });
    let pick = move || data.select_row(row);
    let narrow = narrow_chrome();
    let wide = create_memo(move || !narrow.get());
    let (wide_marks, wide_parts, wide_keys) = (marks, parts, part_keys);
    let (wide_entry, narrow_entry) = (entry, entry);
    let (wide_code, wide_title) = (code, title);
    view! {
        <ListRow
            selected={selected}
            @test_id={format!("database-view.list.row.{}", row.as_uuid())}
            on_click={pick}
            on_activate={pick}
        >
            <List spacing=0.0>
                <Show condition={wide}>
                    {move || view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Frame @sizing=ItemSize::Fixed(CODE_WIDTH)>
                                <Text style=TextStyle::CAPTION string={wide_code} ellipsis=true />
                            </Frame>
                            <Marks data={data} entry={wide_entry} marks={wide_marks} />
                            <Text
                                @sizing={ItemSize::Percent(100.0).shrink(1.0)}
                                string={wide_title}
                                ellipsis=true
                            />
                            <Parts parts={wide_parts} keys={wide_keys} />
                        </List>
                    }}
                </Show>
                <Show condition={narrow}>
                    {move || view! {
                        <List spacing=4.0>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Start
                                spacing=SPACING
                            >
                                <Marks data={data} entry={narrow_entry} marks={marks} />
                                <Text @sizing=ItemSize::Percent(100.0) string={title} wrap=true />
                            </List>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=8.0
                                wrap=true
                            >
                                <Text style=TextStyle::CAPTION string={code} />
                                <Parts parts={parts} keys={part_keys} />
                            </List>
                        </List>
                    }}
                </Show>
            </List>
        </ListRow>
    }
}

#[component]
fn Marks(data: Data, entry: Memo<Option<DatabaseRow>>, marks: Memo<Vec<Uuid>>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <ForEach keys={marks}>
                {move |field: Uuid| {
                    let option = create_memo(move || {
                        let chosen = entry.with(|entry| match entry.as_ref()?.get(field) {
                            Some(DatabaseValue::Enum(option)) => Some(option),
                            _ => None,
                        })?;
                        data.fields.with(|fields| {
                            fields
                                .iter()
                                .find(|known| known.id == field)
                                .and_then(|known| known.option(chosen).cloned())
                        })
                    });
                    let set = create_memo(move || option.with(Option::is_some));
                    view! {
                        <Frame width=MARK_SIZE height=MARK_SIZE>
                            <Frame visible={set}>
                                <ChoiceMark option={option} size=MARK />
                            </Frame>
                        </Frame>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
pub fn Parts(parts: Memo<Vec<(Uuid, Part)>>, keys: Memo<Vec<Uuid>>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <ForEach keys={keys}>
                {move |key: Uuid| {
                    let part = create_memo(move || {
                        parts.with(|parts| {
                            parts.iter().find(|(held, _)| *held == key).map(|(_, part)| part.clone())
                        })
                    });
                    view! {
                        <RowPart part={part} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn RowPart(part: Memo<Option<Part>>) -> NodeId {
    let theme = use_theme();
    let text = create_memo(move || match part.get() {
        Some(Part::Choice(option)) => option.name,
        Some(Part::Link(label)) | Some(Part::Text(label)) => label,
        Some(Part::Links(name, count)) => format!("{name} {count}"),
        Some(Part::Page) | None => String::new(),
    });
    let page = create_memo(move || matches!(part.get(), Some(Part::Page)));
    let chip = create_memo(move || {
        matches!(
            part.get(),
            Some(Part::Choice(_) | Part::Link(_) | Part::Links(..))
        )
    });
    let plain = create_memo(move || matches!(part.get(), Some(Part::Text(_))));
    let dot = create_memo(move || match part.get() {
        Some(Part::Choice(option)) => choice_color(option.color, theme.kind.get()),
        _ => theme.text_muted.get(),
    });
    let dotted = create_memo(move || matches!(part.get(), Some(Part::Choice(_))));
    let linked = create_memo(move || matches!(part.get(), Some(Part::Links(..))));
    let plain_text = text;
    let (page_color, link_color) = (TextColor::Muted, TextColor::Muted);
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame visible={page}>
                <Text size=MARK color={page_color} align=TextAlign::Center>
                    <Icon glyph={ICON_DESCRIPTION.to_owned()} />
                </Text>
            </Frame>
            <Frame visible={plain}>
                <Text style=TextStyle::CAPTION string={plain_text} ellipsis=true />
            </Frame>
            <Frame
                visible={chip}
                outline={theme.border}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=10
                padding_horizontal=8.0
                padding_vertical=1.0
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=5.0>
                    <Frame visible={dotted}>
                        <Frame width=8.0 height=8.0 radius=2 color={dot} />
                    </Frame>
                    <Frame visible={linked}>
                        <Text size=TextSize::Xs color={link_color} align=TextAlign::Center>
                            <Icon glyph={ICON_LINK.to_owned()} />
                        </Text>
                    </Frame>
                    <Text style=TextStyle::CAPTION string={text} align=TextAlign::Start />
                </List>
            </Frame>
        </List>
    }
}

pub fn iconic(field: &DatabaseField) -> bool {
    field.field_type == DatabaseFieldType::Enum
        && field
            .enum_options
            .iter()
            .any(|option| option.icon != ChoiceIcon::None)
}

pub fn row_title(entry: Option<&DatabaseRow>, field: Option<Uuid>) -> String {
    let text = field
        .zip(entry)
        .and_then(|(field, entry)| match entry.value(field) {
            Some(DatabaseValue::String(text)) if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        });
    text.unwrap_or_else(|| "Untitled".to_owned())
}

pub fn row_parts(
    entry: &DatabaseRow,
    fields: &[DatabaseField],
    hidden: &[Uuid],
    labels: &BlockLabels,
    title: Option<Uuid>,
) -> Vec<(Uuid, Part)> {
    let mut parts: Vec<(Uuid, Part)> = fields
        .iter()
        .filter(|field| Some(field.id) != title && !hidden.contains(&field.id) && !iconic(field))
        .filter_map(|field| {
            let value = entry.get(field.id)?;
            let part = match (field.field_type, value) {
                (DatabaseFieldType::Enum, DatabaseValue::Enum(option)) => {
                    Part::Choice(field.option(option)?.clone())
                }
                (DatabaseFieldType::Relation, DatabaseValue::Relation(link)) => {
                    Part::Link(labels.row(&link))
                }
                (DatabaseFieldType::Relation, DatabaseValue::Links(links))
                    if field.relation_options.multiple =>
                {
                    Part::Links(field.name.clone(), links.len())
                }
                (DatabaseFieldType::Relation, DatabaseValue::Links(links)) => {
                    Part::Link(labels.row(links.first()?))
                }
                (DatabaseFieldType::Boolean, DatabaseValue::Boolean(false)) => return None,
                (DatabaseFieldType::Boolean, DatabaseValue::Boolean(true)) => {
                    Part::Text(field.name.clone())
                }
                _ => {
                    let text = cell_text(entry, field, labels);
                    if text.trim().is_empty() {
                        return None;
                    }
                    Part::Text(text)
                }
            };
            Some((field.id, part))
        })
        .collect();
    if entry.page().is_some() {
        parts.push((PAGE_PART, Part::Page));
    }
    parts
}
