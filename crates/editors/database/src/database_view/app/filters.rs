use std::collections::BTreeMap;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::database::{DatabaseColor, DatabaseValue};
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::be_block::database_view::{DatabaseView, FilterCondition, ViewFilter};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_DELETE, ICON_FILTER_LIST};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Show, Spacer, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, Heading, IconButton, Select, TextInput,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::block_ui::database::{
    DatabaseBlockPickRequest, DatabaseValueChange, ValueLabels,
};
use block_editor_beui::database::DatabaseValueEditor;

use crate::database_view::app::data::Data;
use crate::database_view::app::sidebar::FieldPicker;

const SPACING: f32 = 6.0;
const CONDITIONS: [&str; 5] = ["is set", "is empty", "is", "is not", "contains"];

#[component]
pub fn Filters(data: Data) -> NodeId {
    let keys = create_memo(clone!(data -> move || {
        data.filter_items
            .with(|items| items.iter().map(|(id, _)| *id).collect::<Vec<ObjectId>>())
    }));
    let none = create_memo(clone!(keys -> move || keys.with(Vec::is_empty)));
    let add = clone!(data -> move || {
        let field = data.fields.with_untracked(|fields| fields.first().map(|field| field.id));
        data.operate_view(DatabaseView::add_filter(field, FilterCondition::IsSet).1);
    });
    let rows = data.clone();
    let read_only = data.read_only.clone();
    view! {
        <List spacing=SPACING>
            <Heading content="Filters" />
            <Show condition={none}>
                <Caption content="Every row is shown." wrap=true />
            </Show>
            <ForEach keys={keys}>
                {move |id: ObjectId| {
                    let data = rows.clone();
                    view! {
                        <FilterRow data={data} id={id} />
                    }
                }}
            </ForEach>
            <List direction=Direction::Horizontal spacing=0.0>
                <Button
                    glyph={ICON_FILTER_LIST.to_owned()}
                    label="Add filter"
                    variant=ButtonVariant::Secondary
                    disabled={read_only}
                    @test_id={"database-view.filters.add"}
                    on_click={add}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </List>
    }
}

#[component]
fn FilterRow(data: Data, id: ObjectId) -> NodeId {
    let filter = create_memo(clone!(data -> move || {
        data.filter_items.with(|items| {
            items
                .iter()
                .find(|(held, _)| *held == id)
                .map(|(_, filter)| filter.clone())
                .unwrap_or_default()
        })
    }));
    let chosen = create_memo(clone!(filter -> move || filter.with(|filter| filter.field)));
    let field = create_memo(clone!(data chosen -> move || {
        let chosen = chosen.get()?;
        data.fields
            .with(|fields| fields.iter().find(|field| field.id == chosen).cloned())
    }));
    let condition = create_memo(clone!(filter -> move || condition_index(&filter.get())));
    let compared = create_memo(clone!(condition -> move || matches!(condition.get(), Some(2 | 3))));
    let searched = create_memo(clone!(condition -> move || condition.get() == Some(4)));
    let fields = data.fields.clone();
    let refield = clone!(data fields -> move |index: Option<usize>| {
        let field = index.and_then(|index| {
            fields.with_untracked(|fields| fields.get(index).map(|field| field.id))
        });
        data.operate_view(DatabaseView::set_filter_field(id, field));
        data.operate_view(DatabaseView::set_filter_condition(id, &FilterCondition::IsSet));
    });
    let recondition = clone!(data filter field -> move |index: Option<usize>| {
        let Some(index) = index else {
            return;
        };
        let current = filter.get_untracked().condition;
        let labels = data.labels.get_untracked();
        let next = field
            .get_untracked()
            .and_then(|field| next_condition(index, &current, &field, &labels));
        if let Some(next) = next {
            data.operate_view(DatabaseView::set_filter_condition(id, &next));
        }
    });
    let remove = clone!(data -> move || data.operate_view(DatabaseView::remove_filter(id)));
    let value_fields =
        create_memo(clone!(field -> move || field.get().into_iter().collect::<Vec<_>>()));
    let values = create_memo(clone!(filter -> move || {
        let filter = filter.get();
        match (filter.field, filter.condition) {
            (Some(field), FilterCondition::Is(value) | FilterCondition::IsNot(value)) => {
                BTreeMap::from([(field, value)])
            }
            _ => BTreeMap::new(),
        }
    }));
    let revalue = clone!(data filter -> move |change: DatabaseValueChange| {
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
    let needle = create_memo(clone!(filter -> move || match filter.get().condition {
        FilterCondition::Contains(text) => text,
        _ => String::new(),
    }));
    let searched_for = clone!(data -> move |text: String| {
        data.operate_view(DatabaseView::set_filter_condition(id, &FilterCondition::Contains(text)));
    });
    let condition_keys = create_memo(|| (0..CONDITIONS.len()).collect::<Vec<usize>>());
    let (remove_off, condition_off, value_off, text_off) = (
        data.read_only.clone(),
        data.read_only.clone(),
        data.read_only.clone(),
        data.read_only.clone(),
    );
    let labels = data.labels.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <FieldPicker
                    @sizing=ItemSize::Percent(100.0)
                    options={fields}
                    chosen={chosen}
                    label="Field"
                    picker_id={format!("database-view.filter.{}.field", id.as_uuid())}
                    on_change={refield}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Remove filter"
                    disabled={remove_off}
                    @test_id={format!("database-view.filter.{}.remove", id.as_uuid())}
                    on_click={remove}
                />
            </List>
            <Select
                options={view! {
                    <ForEach keys={condition_keys}>
                        {|index: usize| view! {
                            <ChoiceOption label={CONDITIONS[index]} />
                        }}
                    </ForEach>
                }}
                selected={condition}
                label="Condition"
                disabled={condition_off}
                @test_id={format!("database-view.filter.{}.condition", id.as_uuid())}
                on_change={recondition}
            />
            <Show condition={compared}>
                <DatabaseValueEditor
                    fields={value_fields.clone()}
                    values={values.clone()}
                    labels={labels.clone()}
                    disabled={value_off.clone()}
                    prefix={format!("database-view.filter.{}", id.as_uuid())}
                    headings=false
                    on_change={revalue.clone()}
                    on_pick={picked}
                />
            </Show>
            <Show condition={searched}>
                <TextInput
                    value={needle.clone()}
                    placeholder="Text"
                    disabled={text_off.clone()}
                    @test_id={format!("database-view.filter.{}.text", id.as_uuid())}
                    on_change={searched_for.clone()}
                />
            </Show>
        </List>
    }
}

fn condition_index(filter: &ViewFilter) -> Option<usize> {
    filter.field?;
    Some(match filter.condition {
        FilterCondition::IsSet => 0,
        FilterCondition::IsEmpty => 1,
        FilterCondition::Is(_) => 2,
        FilterCondition::IsNot(_) => 3,
        FilterCondition::Contains(_) => 4,
    })
}

fn next_condition(
    index: usize,
    current: &FilterCondition,
    field: &DatabaseField,
    labels: &ValueLabels,
) -> Option<FilterCondition> {
    let value = match current {
        FilterCondition::Is(value) | FilterCondition::IsNot(value) => Some(value.clone()),
        _ => None,
    }
    .or_else(|| starting_value(field, labels));
    match index {
        0 => Some(FilterCondition::IsSet),
        1 => Some(FilterCondition::IsEmpty),
        2 => value.map(FilterCondition::Is),
        3 => value.map(FilterCondition::IsNot),
        4 => (field.field_type == DatabaseFieldType::String)
            .then(|| FilterCondition::Contains(String::new())),
        _ => None,
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
