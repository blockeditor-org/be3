use std::collections::BTreeMap;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_LIST_FILTER, ICON_PLUS, ICON_X};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::popover::PANEL_PADDING;
use block_editor_beui::beui::styled::*;
use block_editor_database::{ChoiceMark, DatabaseBlockPickRequest, DatabaseValueEditor};
use block_ui_database::{DatabaseValueChange, ValueLabels, database_value_text};
use database_types::database::{DatabaseColor, DatabaseValue};
use database_types::database_schema::{DatabaseEnumOption, DatabaseField, DatabaseFieldType};
use database_types::database_view::{DatabaseView, FilterCondition, ViewFilter};
use uuid::Uuid;

use crate::database_view::app::controls::MARK;
use crate::database_view::app::data::Data;
use beui::{Align, Direction, ItemSize};
use block_editor_beui::beui::TextAlign;

const SPACING: Space = Space::Sm;
const EDITOR_WIDTH: f32 = 300.0;
const EDITOR_POPOVER_WIDTH: Size = Size::Custom(EDITOR_WIDTH + PANEL_PADDING.px() * 2.0);
const CHOICES_HEIGHT: Size = Size::Custom(280.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    AnyOf,
    NoneOf,
    Open,
    Closed,
    Set,
    Empty,
    Is,
    IsNot,
    Contains,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::AnyOf | Self::Is => "is",
            Self::NoneOf | Self::IsNot => "is not",
            Self::Open => "is open",
            Self::Closed => "is closed",
            Self::Set => "is set",
            Self::Empty => "is empty",
            Self::Contains => "contains",
        }
    }

    fn of(condition: &FilterCondition) -> Self {
        match condition {
            FilterCondition::AnyOf(_) => Self::AnyOf,
            FilterCondition::NoneOf(_) => Self::NoneOf,
            FilterCondition::IsOpen => Self::Open,
            FilterCondition::IsClosed => Self::Closed,
            FilterCondition::IsSet => Self::Set,
            FilterCondition::IsEmpty => Self::Empty,
            FilterCondition::Is(_) => Self::Is,
            FilterCondition::IsNot(_) => Self::IsNot,
            FilterCondition::Contains(_) => Self::Contains,
        }
    }
}

pub fn kinds_for(field: &DatabaseField) -> Vec<Kind> {
    match field.field_type {
        DatabaseFieldType::Enum => vec![
            Kind::AnyOf,
            Kind::NoneOf,
            Kind::Open,
            Kind::Closed,
            Kind::Set,
            Kind::Empty,
        ],
        DatabaseFieldType::Relation => vec![Kind::AnyOf, Kind::NoneOf, Kind::Set, Kind::Empty],
        DatabaseFieldType::String => vec![Kind::Contains, Kind::Set, Kind::Empty],
        _ => vec![Kind::Is, Kind::IsNot, Kind::Set, Kind::Empty],
    }
}

pub fn starting_condition(field: &DatabaseField) -> FilterCondition {
    match field.field_type {
        DatabaseFieldType::Enum | DatabaseFieldType::Relation => FilterCondition::AnyOf(Vec::new()),
        DatabaseFieldType::String => FilterCondition::Contains(String::new()),
        DatabaseFieldType::Boolean => FilterCondition::Is(DatabaseValue::Boolean(true)),
        _ => FilterCondition::IsSet,
    }
}

fn converted(
    kind: Kind,
    current: &FilterCondition,
    field: &DatabaseField,
    labels: &ValueLabels,
) -> FilterCondition {
    let chosen = match current {
        FilterCondition::AnyOf(values) | FilterCondition::NoneOf(values) => values.clone(),
        _ => Vec::new(),
    };
    let single = match current {
        FilterCondition::Is(value) | FilterCondition::IsNot(value) => Some(value.clone()),
        _ => None,
    }
    .or_else(|| starting_value(field, labels));
    match kind {
        Kind::AnyOf => FilterCondition::AnyOf(chosen),
        Kind::NoneOf => FilterCondition::NoneOf(chosen),
        Kind::Open => FilterCondition::IsOpen,
        Kind::Closed => FilterCondition::IsClosed,
        Kind::Set => FilterCondition::IsSet,
        Kind::Empty => FilterCondition::IsEmpty,
        Kind::Is => single.map_or(FilterCondition::IsSet, FilterCondition::Is),
        Kind::IsNot => single.map_or(FilterCondition::IsEmpty, FilterCondition::IsNot),
        Kind::Contains => FilterCondition::Contains(String::new()),
    }
}

fn starting_value(field: &DatabaseField, labels: &ValueLabels) -> Option<DatabaseValue> {
    match field.field_type {
        DatabaseFieldType::String => Some(DatabaseValue::String(String::new())),
        DatabaseFieldType::Number => Some(DatabaseValue::Number(0.0)),
        DatabaseFieldType::Enum => field
            .enum_options
            .first()
            .map(|option| DatabaseValue::Enum(option.id)),
        DatabaseFieldType::Boolean => Some(DatabaseValue::Boolean(true)),
        DatabaseFieldType::Color => Some(DatabaseValue::Color(DatabaseColor::default())),
        DatabaseFieldType::Datetime => Some(DatabaseValue::Datetime(0)),
        DatabaseFieldType::Relation => {
            let database = field.relation_options.database?;
            labels
                .rows_of(database)
                .first()
                .map(|(link, _)| DatabaseValue::Relation(*link))
        }
        DatabaseFieldType::Block => None,
    }
}

pub fn summary(filter: &ViewFilter, fields: &[DatabaseField], labels: &ValueLabels) -> String {
    let Some(field) = filter
        .field
        .and_then(|id| fields.iter().find(|field| field.id == id))
    else {
        return "Any row".to_owned();
    };
    let kind = Kind::of(&filter.condition);
    let detail = match &filter.condition {
        FilterCondition::AnyOf(values) | FilterCondition::NoneOf(values) if values.is_empty() => {
            "…".to_owned()
        }
        FilterCondition::AnyOf(values) | FilterCondition::NoneOf(values) => values
            .iter()
            .map(|value| value_name(value, field, labels))
            .collect::<Vec<_>>()
            .join(", "),
        FilterCondition::Is(value) | FilterCondition::IsNot(value) => {
            value_name(value, field, labels)
        }
        FilterCondition::Contains(text) => format!("“{text}”"),
        _ => String::new(),
    };
    [field.name.as_str(), kind.label(), detail.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn value_name(value: &DatabaseValue, field: &DatabaseField, labels: &ValueLabels) -> String {
    match value {
        DatabaseValue::Relation(link) => labels.row(link),
        value => database_value_text(value, field, labels),
    }
}

fn values_of(condition: &FilterCondition) -> Vec<DatabaseValue> {
    match condition {
        FilterCondition::AnyOf(values) | FilterCondition::NoneOf(values) => values.clone(),
        _ => Vec::new(),
    }
}

#[component]
pub fn FilterBar(data: Data) -> NodeId {
    let keys = create_memo(move || {
        data.filter_items
            .with(|items| items.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING wrap=true>
            <ForEach keys={keys}>|id: ObjectId|
                <FilterChip data={data} id={id} />
            </ForEach>
            <AddFilter data={data} />
        </List>
    }
}

#[component]
fn FilterChip(data: Data, id: ObjectId) -> NodeId {
    let text = create_memo(move || {
        let filter = data.filter_items.with(|items| {
            items
                .iter()
                .find(|(held, _)| *held == id)
                .map(|(_, filter)| filter.clone())
        });
        let Some(filter) = filter else {
            return String::new();
        };
        data.fields
            .with(|fields| data.labels.with(|labels| summary(&filter, fields, labels)))
    });
    let remove = move || data.operate_view(DatabaseView::remove_filter(id));
    let read_only = data.read_only;
    view! {
        <Frame fill=ColorRole::Surface border={Border::DEFAULT} radius=Radius::Full>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                <Popover
                    label={text}
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Compact
                    max_width=EDITOR_POPOVER_WIDTH
                    @test_id={format!("database-view.filter.{}", id.as_uuid())}
                >|_: PopoverHandle|
                    <FilterEditor data={data} id={id} />
                </Popover>
                <Button
                    icon_only=true
                    glyph=ICON_X
                    label="Remove filter"
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
                    disabled={read_only}
                    @test_id={format!("database-view.filter.{}.remove", id.as_uuid())}
                    on_click={remove}
                />
            </List>
        </Frame>
    }
}

#[component]
fn AddFilter(data: Data) -> NodeId {
    let adding = create_signal(None::<ObjectId>);
    let choosing = create_memo(move || adding.get().is_none());
    let editing = create_memo(move || adding.get());
    let has_editing = create_memo(move || adding.get().is_some());
    let open = create_signal(false);
    let open = Binding::new(open, move |next: bool| {
        open.set(next);
        if !next {
            adding.set(None);
        }
    });
    let read_only = data.read_only;
    view! {
        <Popover
            label="Filter"
            glyph=ICON_LIST_FILTER
            variant=ButtonVariant::Ghost
            size=ButtonSize::Compact
            disabled={read_only}
            max_width=EDITOR_POPOVER_WIDTH
            @test_id={"database-view.filters.add"}
            open
        >|_: PopoverHandle|
            <List spacing=Space::None>
                <Show condition={choosing}>
                    <FieldChoices data={data} on_pick={move |id: ObjectId| adding.set(Some(id))} />
                </Show>
                <Show condition={has_editing}>|| {
                    let id = editing.get_untracked();
                    match id {
                        Some(id) => view! {
                            <FilterEditor data={data} id={id} />
                        },
                        None => view! {
                            <Frame />
                        },
                    }
                }</Show>
            </List>
        </Popover>
    }
}

#[component]
fn FieldChoices(
    data: Data,
    on_pick: block_editor_beui::beui::reactive::Callback<ObjectId>,
) -> NodeId {
    let keys = create_memo(move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type != DatabaseFieldType::Block)
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    });
    view! {
        <List spacing=Space::Xs2>
            <Text style=TextStyle::CAPTION>"Filter by"</Text>
            <ForEach keys={keys}>|field: Uuid| {
                let name = create_memo(move || {
                    data.fields.with(|fields| {
                        fields
                            .iter()
                            .find(|known| known.id == field)
                            .map_or_else(String::new, |known| known.name.clone())
                    })
                });
                let pick = move || {
                    let Some(known) = data.fields.with_untracked(|fields| {
                        fields.iter().find(|known| known.id == field).cloned()
                    }) else {
                        return;
                    };
                    let (id, edit) =
                        DatabaseView::add_filter(Some(field), starting_condition(&known));
                    data.operate_view(edit);
                    on_pick.call(id);
                };
                view! {
                    <ActionRow
                        label={name}
                        @test_id={format!("database-view.filters.field.{field}")}
                        on_click={pick}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
pub fn FilterEditor(data: Data, id: ObjectId) -> NodeId {
    let filter = create_memo(move || {
        data.filter_items.with(|items| {
            items
                .iter()
                .find(|(held, _)| *held == id)
                .map(|(_, filter)| filter.clone())
                .unwrap_or_default()
        })
    });
    let field = create_memo(move || {
        let chosen = filter.with(|filter| filter.field)?;
        data.fields
            .with(|fields| fields.iter().find(|field| field.id == chosen).cloned())
    });
    let name = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.name.clone())
                .unwrap_or_default()
        })
    });
    let kinds =
        create_memo(move || field.with(|field| field.as_ref().map(kinds_for).unwrap_or_default()));
    let kind = create_memo(move || filter.with(|filter| Kind::of(&filter.condition)));
    let selected = create_memo(move || {
        let kind = kind.get();
        kinds.with(|kinds| kinds.iter().position(|held| *held == kind))
    });
    let kind_keys = create_memo(move || (0..kinds.with(Vec::len)).collect::<Vec<usize>>());
    let recondition = move |index: Option<usize>| {
        let Some(kind) =
            index.and_then(|index| kinds.with_untracked(|kinds| kinds.get(index).copied()))
        else {
            return;
        };
        let Some(field) = field.get_untracked() else {
            return;
        };
        let current = filter.get_untracked().condition;
        let next = data
            .labels
            .with_untracked(|labels| converted(kind, &current, &field, labels));
        data.operate_view(DatabaseView::set_filter_condition(id, &next));
    };
    let choosing = create_memo(move || matches!(kind.get(), Kind::AnyOf | Kind::NoneOf));
    let comparing = create_memo(move || matches!(kind.get(), Kind::Is | Kind::IsNot));
    let searching = create_memo(move || kind.get() == Kind::Contains);
    let needle = create_memo(move || match filter.get().condition {
        FilterCondition::Contains(text) => text,
        _ => String::new(),
    });
    let searched_for = move |text: String| {
        data.operate_view(DatabaseView::set_filter_condition(
            id,
            &FilterCondition::Contains(text),
        ));
    };
    let value_fields = create_memo(move || field.get().into_iter().collect::<Vec<_>>());
    let values = create_memo(move || {
        let filter = filter.get();
        match (filter.field, filter.condition) {
            (Some(field), FilterCondition::Is(value) | FilterCondition::IsNot(value)) => {
                BTreeMap::from([(field, value)])
            }
            _ => BTreeMap::new(),
        }
    });
    let revalue = move |change: DatabaseValueChange| {
        let Some(value) = change.value else {
            return;
        };
        let next = match filter.get_untracked().condition {
            FilterCondition::IsNot(_) => FilterCondition::IsNot(value),
            _ => FilterCondition::Is(value),
        };
        data.operate_view(DatabaseView::set_filter_condition(id, &next));
    };
    let picked = |_: DatabaseBlockPickRequest| {};
    let (condition_off, value_off, text_off) = (data.read_only, data.read_only, data.read_only);
    let labels = data.labels;
    view! {
        <Frame width={Size::Custom(EDITOR_WIDTH)}>
            <List spacing=Space::Md>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text>{name}</Text>
                    <Frame @sizing=ItemSize::Percent(100.0)>
                        <Select
                            options={view! {
                                <ForEach keys={kind_keys}>|index: usize| {
                                    let label = create_memo(move || {
                                        kinds.with(|kinds| {
                                            kinds.get(index).map(|kind| kind.label()).unwrap_or_default().to_owned()
                                        })
                                    });
                                    view! {
                                        <ChoiceOption label={label} />
                                    }
                                }</ForEach>
                            }}
                            selected={Binding::new(selected, recondition)}
                            label="Condition"
                            disabled={condition_off}
                            @test_id={format!("database-view.filter.{}.condition", id.as_uuid())}
                        />
                    </Frame>
                </List>
                <Show condition={choosing}>
                    <ValueChoices data={data} id={id} filter={filter} field={field} />
                </Show>
                <Show condition={comparing}>
                    <DatabaseValueEditor
                        fields={value_fields}
                        values={values}
                        labels={labels}
                        disabled={value_off}
                        prefix={format!("database-view.filter.{}", id.as_uuid())}
                        headings=false
                        on_change={revalue}
                        on_pick={picked}
                    />
                </Show>
                <Show condition={searching}>
                    <TextInput
                        value={Binding::new(needle, searched_for)}
                        placeholder="Text"
                        disabled={text_off}
                        focused=true
                        @test_id={format!("database-view.filter.{}.text", id.as_uuid())}
                    />
                </Show>
            </List>
        </Frame>
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Choice {
    value: DatabaseValue,
    label: String,
    option: Option<DatabaseEnumOption>,
    count: usize,
}

#[component]
fn ValueChoices(
    data: Data,
    id: ObjectId,
    filter: Memo<ViewFilter>,
    field: Memo<Option<DatabaseField>>,
) -> NodeId {
    let query = create_signal(String::new());
    let choices = create_memo(move || {
        let Some(field) = field.get() else {
            return Vec::new();
        };
        let others: Vec<ViewFilter> = data.filter_items.with(|items| {
            items
                .iter()
                .filter(|(held, _)| *held != id)
                .map(|(_, filter)| filter.clone())
                .collect()
        });
        let query = query.get().trim().to_lowercase();
        let _ = filter.get();
        data.rows.with(|rows| {
            data.fields.with(|fields| {
                data.labels.with(|labels| {
                    let kept: Vec<_> = rows
                        .iter()
                        .filter(|row| others.iter().all(|other| other.matches(row, fields)))
                        .collect();
                    let count = |value: &DatabaseValue| {
                        kept.iter()
                            .filter(|row| {
                                FilterCondition::AnyOf(vec![value.clone()])
                                    .matches(row.get(field.id).as_ref(), Some(&field))
                            })
                            .count()
                    };
                    let all: Vec<Choice> = match field.field_type {
                        DatabaseFieldType::Enum => field
                            .enum_options
                            .iter()
                            .map(|option| {
                                let value = DatabaseValue::Enum(option.id);
                                Choice {
                                    count: count(&value),
                                    value,
                                    label: option.name.clone(),
                                    option: Some(option.clone()),
                                }
                            })
                            .collect(),
                        _ => field
                            .relation_options
                            .database
                            .map(|database| labels.rows_of(database))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(link, label)| {
                                let value = DatabaseValue::Relation(link);
                                Choice {
                                    count: count(&value),
                                    value,
                                    label,
                                    option: None,
                                }
                            })
                            .collect(),
                    };
                    all.into_iter()
                        .filter(|choice| {
                            query.is_empty() || choice.label.to_lowercase().contains(&query)
                        })
                        .collect()
                })
            })
        })
    });
    let many = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .is_some_and(|field| field.field_type == DatabaseFieldType::Relation)
        })
    });
    let keys =
        create_memo(move || choices.with(|choices| (0..choices.len()).collect::<Vec<usize>>()));
    let read_only = data.read_only;
    let typed = move |text: String| query.set(text);
    view! {
        <List spacing=SPACING>
            <Show condition={many}>
                <TextInput
                    value={Binding::new(query, typed)}
                    placeholder="Search"
                    @test_id={format!("database-view.filter.{}.search", id.as_uuid())}
                />
            </Show>
            <Scroll max_length=CHOICES_HEIGHT>
                <List spacing=Space::Xs2>
                    <ForEach keys={keys}>|index: usize| {
                        let choice = create_memo(move || choices.with(|choices| choices.get(index).cloned()));
                        let label = create_memo(move || choice.with(|choice| choice.as_ref().map(|choice| choice.label.clone()).unwrap_or_default()));
                        let count = create_memo(move || choice.with(|choice| choice.as_ref().map_or(0, |choice| choice.count)).to_string());
                        let option = create_memo(move || choice.with(|choice| choice.as_ref().and_then(|choice| choice.option.clone())));
                        let marked = create_memo(move || option.with(Option::is_some));
                        let checked = create_memo(move || {
                            let Some(choice) = choice.get() else {
                                return false;
                            };
                            values_of(&filter.get().condition).contains(&choice.value)
                        });
                        let toggle = move |on: bool| {
                            let Some(choice) = choice.get_untracked() else {
                                return;
                            };
                            let current = filter.get_untracked().condition;
                            let mut values = values_of(&current);
                            values.retain(|held| *held != choice.value);
                            if on {
                                values.push(choice.value);
                            }
                            let next = match current {
                                FilterCondition::NoneOf(_) => FilterCondition::NoneOf(values),
                                _ => FilterCondition::AnyOf(values),
                            };
                            data.operate_view(DatabaseView::set_filter_condition(id, &next));
                        };
                        let element_test_id = format!("database-view.filter.{}.choice.{index}", id.as_uuid());
                        view! {
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=SPACING
                            >
                                <Show condition={marked}>
                                    <ChoiceMark option={option} size=MARK />
                                </Show>
                                <Checkbox
                                    @sizing={block_editor_beui::beui::ItemSize::Percent(100.0).shrink(1.0)}
                                    label={label}
                                    checked={Binding::new(checked, toggle)}
                                    disabled={read_only}
                                    @test_id={element_test_id}
                                />
                                <Text style=TextStyle::CAPTION>{count}</Text>
                            </List>
                        }
                    }</ForEach>
                </List>
            </Scroll>
        </List>
    }
}

#[component]
pub fn FilterList(data: Data) -> NodeId {
    let keys = create_memo(move || {
        data.filter_items
            .with(|items| items.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let names = data.fields;
    let field_keys =
        create_memo(move || names.with(|fields| (0..fields.len()).collect::<Vec<usize>>()));
    let add = move |index: Option<usize>| {
        let Some(field) = index.and_then(|index| {
            data.fields
                .with_untracked(|fields| fields.get(index).cloned())
        }) else {
            return;
        };
        data.operate_view(DatabaseView::add_filter(Some(field.id), starting_condition(&field)).1);
    };
    let read_only = data.read_only;
    view! {
        <List spacing=Space::Lg>
            <Show condition={none}>
                <Text style=TextStyle::CAPTION>"Every row is shown."</Text>
            </Show>
            <ForEach keys={keys}>|id: ObjectId| {
                let remove = move || data.operate_view(DatabaseView::remove_filter(id));
                let read_only = data.read_only;
                view! {
                    <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                        <FilterEditor @sizing=ItemSize::Percent(100.0) data={data} id={id} />
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Remove filter"
                            variant=ButtonVariant::Ghost
                            disabled={read_only}
                            @test_id={format!("database-view.filter.{}.remove", id.as_uuid())}
                            on_click={remove}
                        />
                    </List>
                }
            }</ForEach>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_PLUS />
                </Text>
                <Frame @sizing=ItemSize::Percent(100.0)>
                    <Select
                        options={view! {
                            <ForEach keys={field_keys}>|index: usize| {
                                let label = create_memo(move || {
                                    names.with(|fields| fields.get(index).map(|field| field.name.clone()).unwrap_or_default())
                                });
                                view! {
                                    <ChoiceOption label={label} />
                                }
                            }</ForEach>
                        }}
                        selected={Binding::new(None, add)}
                        label="Add a filter"
                        placeholder="Add a filter"
                        disabled={read_only}
                        @test_id={"database-view.filters.add-field"}
                    />
                </Frame>
            </List>
        </List>
    }
}
