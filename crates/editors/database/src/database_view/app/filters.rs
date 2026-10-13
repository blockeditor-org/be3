use std::collections::BTreeMap;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{DatabaseColor, DatabaseValue};
use block_editor_beui::be_block::database_schema::{
    DatabaseEnumOption, DatabaseField, DatabaseFieldType,
};
use block_editor_beui::be_block::database_view::{DatabaseView, FilterCondition, ViewFilter};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ADD, ICON_CLOSE, ICON_FILTER_LIST};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component, create_memo,
    create_signal, view,
};
use block_editor_beui::beui::styled::popover::PopoverPanel;
use block_editor_beui::beui::styled::theme::{BORDER_WIDTH, FONT_SMALL};
use block_editor_beui::beui::styled::{
    Body, ButtonVariant, Caption, Checkbox, IconButton, IconButtonSize, IconSized, Scroll, Select,
    TextInput, use_theme,
};
use block_editor_beui::beui::unstyled::{self, ChoiceOption, PopoverHandle, PopoverTriggerHandle};
use block_editor_beui::block_ui::database::{
    DatabaseBlockPickRequest, DatabaseValueChange, ValueLabels, database_value_text,
};
use block_editor_beui::database::{ChoiceMark, DatabaseValueEditor};
use uuid::Uuid;

use crate::database_view::app::controls::{ControlFace, MARK_SIZE};
use crate::database_view::app::data::Data;

const SPACING: f32 = 6.0;
const EDITOR_WIDTH: f32 = 300.0;
const CHOICES_HEIGHT: f32 = 280.0;

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
    let keys = create_memo(clone!(data -> move || {
        data.filter_items
            .with(|items| items.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
    }));
    let chips = data.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING wrap=true>
            <ForEach keys={keys}>
                {move |id: ObjectId| {
                    let data = chips.clone();
                    view! {
                        <FilterChip data={data} id={id} />
                    }
                }}
            </ForEach>
            <AddFilter data={data} />
        </List>
    }
}

#[component]
fn FilterChip(data: Data, id: ObjectId) -> NodeId {
    let text = create_memo(clone!(data -> move || {
        let filter = data.filter_items.with(|items| {
            items.iter().find(|(held, _)| *held == id).map(|(_, filter)| filter.clone())
        });
        let Some(filter) = filter else {
            return String::new();
        };
        data.fields
            .with(|fields| data.labels.with(|labels| summary(&filter, fields, labels)))
    }));
    let remove = clone!(data -> move || data.operate_view(DatabaseView::remove_filter(id)));
    let editor = data.clone();
    let read_only = data.read_only;
    let theme = use_theme();
    let label = text;
    view! {
        <Frame
            color={theme.surface}
            outline={theme.border}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=14
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                <unstyled::Popover
                    label={label}
                    @test_id={format!("database-view.filter.{}", id.as_uuid())}
                    trigger={move |handle: PopoverTriggerHandle| {
                        let text = text;
                        view! {
                            <ControlFace handle={handle.button}>
                                <Caption content={text} />
                            </ControlFace>
                        }
                    }}
                >
                    {move |_: PopoverHandle| {
                        let data = editor.clone();
                        view! {
                            <PopoverPanel max_width={Some(EDITOR_WIDTH + 24.0)}>
                                <FilterEditor data={data} id={id} />
                            </PopoverPanel>
                        }
                    }}
                </unstyled::Popover>
                <IconButton
                    glyph={ICON_CLOSE.to_owned()}
                    label="Remove filter"
                    variant=ButtonVariant::Ghost
                    size=IconButtonSize::Compact
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
    let (adding, set_adding) = create_signal(None::<ObjectId>);
    let choosing = create_memo(move || adding.get().is_none());
    let editing = create_memo(move || adding.get());
    let has_editing = create_memo(move || adding.get().is_some());
    let reset = move |open: bool| {
        if !open {
            set_adding.set(None);
        }
    };
    let picks = data.clone();
    let edits = data.clone();
    let read_only = data.read_only;
    view! {
        <unstyled::Popover
            label="Filter"
            disabled={read_only}
            @test_id={"database-view.filters.add"}
            on_open_change={reset}
            trigger={move |handle: PopoverTriggerHandle| view! {
                <ControlFace handle={handle.button}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <IconSized
                            glyph={ICON_FILTER_LIST.to_owned()}
                            font_size={FONT_SMALL + 2.0}
                            color={use_theme().text_muted}
                        />
                        <Caption content="Filter" />
                    </List>
                </ControlFace>
            }}
        >
            {move |_: PopoverHandle| {
                let picks = picks.clone();
                let edits = edits.clone();
                let set_adding = set_adding;
                let editing = editing;
                view! {
                    <PopoverPanel max_width={Some(EDITOR_WIDTH + 24.0)}>
                        <List spacing=0.0>
                            <Show condition={choosing}>
                                {move || clone!(picks -> view! {
                                    <FieldChoices
                                        data={picks}
                                        on_pick={move |id: ObjectId| set_adding.set(Some(id))}
                                    />
                                })}
                            </Show>
                            <Show condition={has_editing}>
                                {move || {
                                    let id = editing.get_untracked();
                                    let data = edits.clone();
                                    match id {
                                        Some(id) => view! {
                                            <FilterEditor data={data} id={id} />
                                        },
                                        None => view! {
                                            <Frame />
                                        },
                                    }
                                }}
                            </Show>
                        </List>
                    </PopoverPanel>
                }
            }}
        </unstyled::Popover>
    }
}

#[component]
fn FieldChoices(
    data: Data,
    on_pick: block_editor_beui::beui::reactive::Callback<ObjectId>,
) -> NodeId {
    let keys = create_memo(clone!(data -> move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type != DatabaseFieldType::Block)
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    }));
    view! {
        <List spacing=2.0>
            <Caption content="Filter by" />
            <ForEach keys={keys}>
                {move |field: Uuid| {
                    let data = data.clone();
                    let on_pick = on_pick;
                    let name = create_memo(clone!(data -> move || {
                        data.fields.with(|fields| {
                            fields
                                .iter()
                                .find(|known| known.id == field)
                                .map_or_else(String::new, |known| known.name.clone())
                        })
                    }));
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
                        <unstyled::Button
                            label={name}
                            @test_id={format!("database-view.filters.field.{field}")}
                            on_click={pick}
                            content={move |handle| {
                                let name = name;
                                view! {
                                    <ControlFace handle={handle}>
                                        <Body content={name} />
                                    </ControlFace>
                                }
                            }}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
pub fn FilterEditor(data: Data, id: ObjectId) -> NodeId {
    let filter = create_memo(clone!(data -> move || {
        data.filter_items.with(|items| {
            items
                .iter()
                .find(|(held, _)| *held == id)
                .map(|(_, filter)| filter.clone())
                .unwrap_or_default()
        })
    }));
    let field = create_memo(clone!(data -> move || {
        let chosen = filter.with(|filter| filter.field)?;
        data.fields
            .with(|fields| fields.iter().find(|field| field.id == chosen).cloned())
    }));
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
    let recondition = clone!(data -> move |index: Option<usize>| {
        let Some(kind) = index.and_then(|index| kinds.with_untracked(|kinds| kinds.get(index).copied())) else {
            return;
        };
        let Some(field) = field.get_untracked() else {
            return;
        };
        let current = filter.get_untracked().condition;
        let next = data.labels.with_untracked(|labels| converted(kind, &current, &field, labels));
        data.operate_view(DatabaseView::set_filter_condition(id, &next));
    });
    let choosing = create_memo(move || matches!(kind.get(), Kind::AnyOf | Kind::NoneOf));
    let comparing = create_memo(move || matches!(kind.get(), Kind::Is | Kind::IsNot));
    let searching = create_memo(move || kind.get() == Kind::Contains);
    let needle = create_memo(move || match filter.get().condition {
        FilterCondition::Contains(text) => text,
        _ => String::new(),
    });
    let searched_for = clone!(data -> move |text: String| {
        data.operate_view(DatabaseView::set_filter_condition(id, &FilterCondition::Contains(text)));
    });
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
    let revalue = clone!(data -> move |change: DatabaseValueChange| {
        let Some(value) = change.value else {
            return;
        };
        let next = match filter.get_untracked().condition {
            FilterCondition::IsNot(_) => FilterCondition::IsNot(value),
            _ => FilterCondition::Is(value),
        };
        data.operate_view(DatabaseView::set_filter_condition(id, &next));
    });
    let picked = |_: DatabaseBlockPickRequest| {};
    let (condition_off, value_off, text_off) = (data.read_only, data.read_only, data.read_only);
    let labels = data.labels;
    let choices = data.clone();
    let choice_filter = filter;
    let choice_field = field;
    let condition_names = kinds;
    view! {
        <Frame width=EDITOR_WIDTH>
            <List spacing={SPACING * 1.5}>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Body content={name} />
                    <Frame @sizing=ItemSize::Percent(100.0)>
                        <Select
                            options={view! {
                                <ForEach keys={kind_keys}>
                                    {move |index: usize| {
                                        let label = create_memo(move || {
                                            condition_names.with(|kinds| {
                                                kinds.get(index).map(|kind| kind.label()).unwrap_or_default().to_owned()
                                            })
                                        });
                                        view! {
                                            <ChoiceOption label={label} />
                                        }
                                    }}
                                </ForEach>
                            }}
                            selected={selected}
                            label="Condition"
                            disabled={condition_off}
                            @test_id={format!("database-view.filter.{}.condition", id.as_uuid())}
                            on_change={recondition}
                        />
                    </Frame>
                </List>
                <Show condition={choosing}>
                    {move || clone!(choices -> view! {
                        <ValueChoices
                            data={choices}
                            id={id}
                            filter={choice_filter}
                            field={choice_field}
                        />
                    })}
                </Show>
                <Show condition={comparing}>
                    <DatabaseValueEditor
                        fields={value_fields}
                        values={values}
                        labels={labels}
                        disabled={value_off}
                        prefix={format!("database-view.filter.{}", id.as_uuid())}
                        headings=false
                        on_change={revalue.clone()}
                        on_pick={picked}
                    />
                </Show>
                <Show condition={searching}>
                    <TextInput
                        value={needle}
                        placeholder="Text"
                        disabled={text_off}
                        focused=true
                        @test_id={format!("database-view.filter.{}.text", id.as_uuid())}
                        on_change={searched_for.clone()}
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
    let (query, set_query) = create_signal(String::new());
    let choices = create_memo(clone!(data -> move || {
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
                            .filter(|row| FilterCondition::AnyOf(vec![value.clone()]).matches(row.get(field.id).as_ref(), Some(&field)))
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
                        .filter(|choice| query.is_empty() || choice.label.to_lowercase().contains(&query))
                        .collect()
                })
            })
        })
    }));
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
    let typed = move |text: String| set_query.set(text);
    view! {
        <List spacing=SPACING>
            <Show condition={many}>
                <TextInput
                    value={query}
                    placeholder="Search"
                    @test_id={format!("database-view.filter.{}.search", id.as_uuid())}
                    on_change={typed}
                />
            </Show>
            <Scroll max_length=CHOICES_HEIGHT>
                <List spacing=2.0>
                    <ForEach keys={keys}>
                        {move |index: usize| {
                            let data = data.clone();
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
                            let test_id = format!("database-view.filter.{}.choice.{index}", id.as_uuid());
                            let read_only = read_only;
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=SPACING
                                >
                                    <Frame visible={marked}>
                                        <ChoiceMark option={option} size=MARK_SIZE />
                                    </Frame>
                                    <Checkbox
                                        @sizing={block_editor_beui::beui::reactive::ItemSize::Percent(100.0).shrink(1.0)}
                                        label={label}
                                        checked={checked}
                                        disabled={read_only}
                                        @test_id={test_id}
                                        on_change={toggle}
                                    />
                                    <Caption content={count} />
                                </List>
                            }
                        }}
                    </ForEach>
                </List>
            </Scroll>
        </List>
    }
}

#[component]
pub fn FilterList(data: Data) -> NodeId {
    let keys = create_memo(clone!(data -> move || {
        data.filter_items
            .with(|items| items.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
    }));
    let none = create_memo(move || keys.with(Vec::is_empty));
    let names = data.fields;
    let field_keys =
        create_memo(move || names.with(|fields| (0..fields.len()).collect::<Vec<usize>>()));
    let add = clone!(data -> move |index: Option<usize>| {
        let Some(field) = index.and_then(|index| data.fields.with_untracked(|fields| fields.get(index).cloned())) else {
            return;
        };
        data.operate_view(DatabaseView::add_filter(Some(field.id), starting_condition(&field)).1);
    });
    let editors = data.clone();
    let read_only = data.read_only;
    view! {
        <List spacing={SPACING * 2.0}>
            <Show condition={none}>
                <Caption content="Every row is shown." />
            </Show>
            <ForEach keys={keys}>
                {move |id: ObjectId| {
                    let data = editors.clone();
                    let remove = clone!(data -> move || data.operate_view(DatabaseView::remove_filter(id)));
                    let read_only = data.read_only;
                    view! {
                        <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                            <FilterEditor @sizing=ItemSize::Percent(100.0) data={data} id={id} />
                            <IconButton
                                glyph={ICON_CLOSE.to_owned()}
                                label="Remove filter"
                                variant=ButtonVariant::Ghost
                                disabled={read_only}
                                @test_id={format!("database-view.filter.{}.remove", id.as_uuid())}
                                on_click={remove}
                            />
                        </List>
                    }
                }}
            </ForEach>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <IconSized
                    glyph={ICON_ADD.to_owned()}
                    font_size=18.0
                    color={use_theme().text_muted}
                />
                <Frame @sizing=ItemSize::Percent(100.0)>
                    <Select
                        options={view! {
                            <ForEach keys={field_keys}>
                                {move |index: usize| {
                                    let label = create_memo(move || {
                                        names.with(|fields| fields.get(index).map(|field| field.name.clone()).unwrap_or_default())
                                    });
                                    view! {
                                        <ChoiceOption label={label} />
                                    }
                                }}
                            </ForEach>
                        }}
                        selected={None}
                        label="Add a filter"
                        placeholder="Add a filter"
                        disabled={read_only}
                        @test_id={"database-view.filters.add-field"}
                        on_change={add}
                    />
                </Frame>
            </List>
        </List>
    }
}
