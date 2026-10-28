use beui::Overflow;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::icons::{ICON_CHEVRON_DOWN, ICON_FILE_TEXT, ICON_LINK, ICON_PLUS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::block_ui::database::cell_text;
use block_editor_beui::database::{ChoiceMark, use_choice_colors};
use block_editor_beui::narrow_chrome;
use database_types::database::{DatabaseRow, DatabaseValue, short_id};
use database_types::database_schema::{
    ChoiceIcon, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
};
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::app::row::RowPanel;
use crate::database_view::groups::{Group, GroupKey};
use crate::database_view::sort::BlockLabels;
use beui::{Align, Direction, ItemSize};

const PANEL_WIDTH: f32 = 460.0;
const LINE_HEIGHT: f32 = 38.0;
const CODE_WIDTH: f32 = 76.0;
const MARK: TextSize = TextSize::Sm;
const MARK_SIZE: f32 = MARK.icon_px();
const SPACING: Space = Space::Md;
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
        groups.with(|groups| {
            let mut lines = Vec::new();
            for group in groups {
                if group.key != GroupKey::All {
                    lines.push(Line::Header(group.key));
                }
                if group.closed != toggled.contains(&group.key) {
                    continue;
                }
                lines.extend(group.rows.iter().copied().map(Line::Row));
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
    let moved = move |moved: ReorderMove<Line, ObjectId>| {
        let row = Line::Row(moved.payload);
        let (group, after) = lines.with_untracked(|lines| {
            let rest: Vec<Line> = lines.iter().copied().filter(|line| *line != row).collect();
            landing(&rest, moved.after, groups)
        });
        data.move_row(moved.payload, group, after);
    };
    let name = move |line: Line| match line {
        Line::Row(row) => row_name(data, row),
        Line::Header(_) => String::new(),
    };
    let preview = move |row: ObjectId| {
        view! {
            <Frame
                width={Size::Custom(PANEL_WIDTH)}
                fill=ColorRole::SurfaceRaised
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Md
                padding={Padding::symmetric(Space::Md, Space::Sm)}
            >
                <Text overflow=Overflow::Ellipsis>{row_name(data, row)}</Text>
            </Frame>
        }
    };
    view! {
        <Frame fill=ColorRole::Background>
            <Reorder
                keys={lines}
                payload={Func::new(|line: Line| match line {
                    Line::Row(row) => Some(row),
                    Line::Header(_) => None,
                })}
                disabled={create_memo(move || !data.reorderable.get())}
                long_press=true
                text={name}
                preview={preview}
                on_move={moved}
            >|reorder: ReorderHandle<Line, ObjectId>|
                <Scroll>
                    <Frame padding={Padding::ZERO.bottom(Space::Xl3)}>
                        <List spacing=Space::None>
                            <Show condition={empty}>
                                <Frame padding={Padding::symmetric(Space::Xl, Space::Lg)}>
                                    <Text style=TextStyle::CAPTION>{blank}</Text>
                                </Frame>
                            </Show>
                            <VirtualList
                                keys={reorder.order}
                                item_size=LINE_HEIGHT
                            >|line: ReadSignal<Line>|
                                <ReorderItem
                                    reorder
                                    item={line}
                                    radius=Radius::None
                                    on_click={move || {
                                        if let Line::Row(row) = line.get_untracked() {
                                            data.select_row(row);
                                        }
                                    }}
                                >|_: ReorderItemHandle|
                                    <AnyLine data={data} groups={groups} line={line} />
                                </ReorderItem>
                            </VirtualList>
                        </List>
                    </Frame>
                </Scroll>
            </Reorder>
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
            && data.kind.get() != database_types::database_view::DatabaseViewKind::Spreadsheet
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
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Layers @sizing=ItemSize::Percent(100.0)>
                <ShowKeepAlive condition={main_shown}>{children}</ShowKeepAlive>
                <Show condition={instead}>
                    <OpenRow data={data} row={selected} />
                </Show>
            </Layers>
            <Show condition={beside}>
                <Frame @sizing=ItemSize::Fixed(PANEL_WIDTH)>
                    <List direction=Direction::Horizontal spacing=Space::None>
                        <Separator direction=Direction::Vertical />
                        <OpenRow @sizing=ItemSize::Percent(100.0) data={data} row={selected} />
                    </List>
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn OpenRow(data: Data, row: Memo<Option<ObjectId>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Dynamic value={row}>|row: Option<ObjectId>| {
                match row {
                    Some(row) => view! {
                        <RowPanel @sizing=ItemSize::Percent(100.0) data={data} row={row} />
                    },
                    None => view! {
                        <Frame @sizing=ItemSize::Percent(100.0) />
                    },
                }
            }</Dynamic>
        </List>
    }
}

fn landing(
    rest: &[Line],
    after: Option<Line>,
    groups: Memo<Vec<Group>>,
) -> (GroupKey, Option<ObjectId>) {
    let first =
        || groups.with_untracked(|groups| groups.first().map_or(GroupKey::All, |group| group.key));
    let Some(after) = after else {
        return (first(), None);
    };
    let upto = rest
        .iter()
        .position(|line| *line == after)
        .map_or(0, |at| at + 1);
    let group = rest[..upto]
        .iter()
        .rev()
        .find_map(|line| match line {
            Line::Header(key) => Some(*key),
            Line::Row(_) => None,
        })
        .unwrap_or_else(first);
    let row = match after {
        Line::Row(row) => Some(row),
        Line::Header(_) => None,
    };
    (group, row)
}

pub fn row_name(data: Data, row: ObjectId) -> String {
    let field = data.title_field.get_untracked();
    let index = data
        .row_index
        .with_untracked(|indices| indices.get(&row).copied());
    data.rows
        .with_untracked(|rows| row_title(index.and_then(|index| rows.get(index)), field))
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
    view! {
        <Frame fill=ColorRole::Surface padding={Padding::symmetric(Space::Sm, Space::Xs2)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_DOWN
                    label={chevron}
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
                    @test_id={format!("database-view.group.{test_key}.toggle")}
                    on_click={toggle}
                />
                <Show condition={marked}>
                    <ChoiceMark option={option} size=MARK />
                </Show>
                <Text>{label}</Text>
                <Text style=TextStyle::CAPTION>{count}</Text>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <Button
                    icon_only=true
                    glyph=ICON_PLUS
                    label="Add a row to this group"
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineKind {
    Header,
    Row,
}

#[component]
fn AnyLine(data: Data, groups: Memo<Vec<Group>>, line: ReadSignal<Line>) -> NodeId {
    let kind = create_memo(move || match line.get() {
        Line::Header(_) => LineKind::Header,
        Line::Row(_) => LineKind::Row,
    });
    let header = create_memo(move || match line.get() {
        Line::Header(key) => Some(key),
        Line::Row(_) => None,
    });
    let row = create_memo(move || match line.get() {
        Line::Row(row) => Some(row),
        Line::Header(_) => None,
    });
    view! {
        <List spacing=Space::None>
            <Dynamic value={kind}>|kind: LineKind| {
                match kind {
                    LineKind::Header => view! {
                        <List spacing=Space::None>
                            <Dynamic value={header}>|key: Option<GroupKey>| {
                                match key {
                                    Some(key) => view! {
                                        <GroupHeader data={data} groups={groups} key={key} />
                                    },
                                    None => view! {
                                        <Frame />
                                    },
                                }
                            }</Dynamic>
                        </List>
                    },
                    LineKind::Row => view! {
                        <RowLine data={data} row={row} />
                    },
                }
            }</Dynamic>
        </List>
    }
}

#[component]
fn RowLine(data: Data, row: Memo<Option<ObjectId>>) -> NodeId {
    let index = create_memo(move || {
        let row = row.get()?;
        data.row_index.with(|indices| indices.get(&row).copied())
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
    let code = create_memo(move || {
        row.get()
            .map(|row| short_id(&data.prefix.get(), row.as_uuid()))
            .unwrap_or_default()
    });
    let element_test_id = create_memo(move || {
        row.get()
            .map(|row| format!("database-view.list.row.{}", row.as_uuid()))
            .unwrap_or_default()
    });
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
    let pick = move || {
        if let Some(row) = row.get_untracked() {
            data.select_row(row);
        }
    };
    let narrow = narrow_chrome();
    let wide = create_memo(move || !narrow.get());
    view! {
        <ListRow selected={selected} @test_id={element_test_id} on_activate={pick}>
            <List spacing=Space::None>
                <Show condition={wide}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Frame @sizing=ItemSize::Fixed(CODE_WIDTH)>
                            <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{code}</Text>
                        </Frame>
                        <Marks data={data} entry={entry} marks={marks} />
                        <Text
                            @sizing={ItemSize::Percent(100.0).shrink(1.0)}
                            overflow=Overflow::Ellipsis
                        >
                            {title}
                        </Text>
                        <Parts parts={parts} keys={part_keys} />
                    </List>
                </Show>
                <Show condition={narrow}>
                    <List spacing=Space::Xs>
                        <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                            <Marks data={data} entry={entry} marks={marks} />
                            <Text @sizing=ItemSize::Percent(100.0)>{title}</Text>
                        </List>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=Space::Md
                            wrap=true
                        >
                            <Text style=TextStyle::CAPTION>{code}</Text>
                            <Parts parts={parts} keys={part_keys} />
                        </List>
                    </List>
                </Show>
            </List>
        </ListRow>
    }
}

#[component]
fn Marks(data: Data, entry: Memo<Option<DatabaseRow>>, marks: Memo<Vec<Uuid>>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <ForEach keys={marks}>|field: Uuid| {
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
                    <Frame width={Size::Custom(MARK_SIZE)} height={Size::Custom(MARK_SIZE)}>
                        <Layers>
                            <Show condition={set}>
                                <ChoiceMark option={option} size=MARK />
                            </Show>
                        </Layers>
                    </Frame>
                }
            }</ForEach>
        </List>
    }
}

#[component]
pub fn Parts(
    parts: Memo<Vec<(Uuid, Part)>>,
    keys: Memo<Vec<Uuid>>,
    #[prop(default = false)] wrap: bool,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm wrap={wrap}>
            <ForEach keys={keys}>|key: Uuid| {
                let part = create_memo(move || {
                    parts.with(|parts| {
                        parts.iter().find(|(held, _)| *held == key).map(|(_, part)| part.clone())
                    })
                });
                view! {
                    <RowPart part={part} />
                }
            }</ForEach>
        </List>
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartKind {
    Page,
    Plain,
    Chip,
}

#[component]
fn RowPart(part: Memo<Option<Part>>) -> NodeId {
    let kind = create_memo(move || {
        part.with(|part| match part {
            Some(Part::Page) | None => PartKind::Page,
            Some(Part::Text(_)) => PartKind::Plain,
            Some(Part::Choice(_) | Part::Link(_) | Part::Links(..)) => PartKind::Chip,
        })
    });
    let text = create_memo(move || {
        part.with(|part| match part {
            Some(Part::Choice(option)) => option.name.clone(),
            Some(Part::Link(label)) | Some(Part::Text(label)) => label.clone(),
            Some(Part::Links(name, count)) => format!("{name} {count}"),
            Some(Part::Page) | None => String::new(),
        })
    });
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Dynamic value={kind}>|kind: PartKind| {
                match kind {
                    PartKind::Page => view! {
                        <Text size=MARK color=TextColor::Muted align=TextAlign::Center>
                            <Icon glyph=ICON_FILE_TEXT />
                        </Text>
                    },
                    PartKind::Plain => view! {
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{text}</Text>
                    },
                    PartKind::Chip => view! {
                        <Chip part={part} text={text} />
                    },
                }
            }</Dynamic>
        </List>
    }
}

#[component]
fn Chip(part: Memo<Option<Part>>, text: Memo<String>) -> NodeId {
    let colors = use_choice_colors();
    let choice = create_memo(move || match part.get() {
        Some(Part::Choice(option)) => Some(option),
        _ => None,
    });
    let dotted = create_memo(move || choice.with(Option::is_some));
    let dot = create_memo(move || match choice.get() {
        Some(option) => ColorRole::Custom(colors.get(option.color)),
        None => ColorRole::TextMuted,
    });
    let linked = create_memo(move || part.with(|part| matches!(part, Some(Part::Links(..)))));
    view! {
        <Frame
            border={Border::DEFAULT}
            radius=Radius::Lg
            padding={Padding::symmetric(Space::Md, Space::Xs2)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                <Show condition={dotted}>
                    <Frame
                        width={Size::Custom(8.0)}
                        height={Size::Custom(8.0)}
                        radius=Radius::Xs
                        fill={dot}
                    />
                </Show>
                <Show condition={linked}>
                    <Text size=TextSize::Xs color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph=ICON_LINK />
                    </Text>
                </Show>
                <Text style=TextStyle::CAPTION align=TextAlign::Start>{text}</Text>
            </List>
        </Frame>
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
