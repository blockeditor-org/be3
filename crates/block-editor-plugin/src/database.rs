use std::collections::{BTreeMap, HashMap};

use beui::icons::ICON_CLEAR;
use beui::reactive::{
    Align, Callback, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, Prop, Show, Spacer,
    clone, component, create_memo, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, ColorInput, IconButton, NumberDrag,
    NumberInput, Select,
    TextInput,
};
use beui::unstyled::ChoiceOption;
use beui::{Color32, NodeId};
use block_client::block_ref::BlockRef;
use block_client::blocks::database::{DatabaseColor, DatabaseValue};
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale,
};
use block_ui::BlockLabel;
use block_ui::database::{
    DatabaseBlockPickRequest, DatabaseValueChange, block_reference_text, field_type_label,
};
use block_ui::datetime::DateTimeFields;
use uuid::Uuid;

use crate::DateTimeRow;

const SPACING: f32 = 6.0;
const FIELD_SPACING: f32 = 12.0;

pub type ValueLabels = HashMap<BlockRef, BlockLabel>;
pub type RowValues = BTreeMap<Uuid, DatabaseValue>;

#[component]
pub fn DatabaseValueEditor(
    fields: Prop<Vec<DatabaseField>>,
    values: Prop<RowValues>,
    #[prop(default = ValueLabels::new())] labels: Prop<ValueLabels>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] prefix: Prop<String>,
    #[prop(default = true)] headings: Prop<bool>,
    on_change: Callback<DatabaseValueChange>,
    on_pick: Callback<DatabaseBlockPickRequest>,
    on_submit: Callback<Uuid>,
) -> NodeId {
    let headings = create_memo(move || headings.get());
    let fields = create_memo(move || fields.get());
    let values = create_memo(move || values.get());
    let labels = create_memo(move || labels.get());
    let disabled = create_memo(move || disabled.get());
    let prefix = create_memo(move || prefix.get());
    let keys = create_memo(clone!(fields -> move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    }));
    view! {
        <List spacing=FIELD_SPACING>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let field = field_of(fields.clone(), id);
                    let value = value_of(values.clone(), id);
                    let test_id = format!("{}.field.{id}", prefix.get_untracked());
                    view! {
                        <DatabaseValueRow
                            field={field}
                            value={value}
                            labels={labels.clone()}
                            headings={headings.clone()}
                            disabled={disabled.clone()}
                            id={test_id}
                            on_change={forward(on_change.clone())}
                            on_pick={forward(on_pick.clone())}
                            on_submit={forward(on_submit.clone())}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn DatabaseValueRow(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    labels: Memo<ValueLabels>,
    headings: Memo<bool>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
    on_pick: Callback<DatabaseBlockPickRequest>,
    on_submit: Callback<Uuid>,
) -> NodeId {
    let heading = create_memo(clone!(field -> move || {
        field.with(|field| {
            field.as_ref().map_or_else(String::new, |field| {
                format!("{} ({})", field.name, field_type_label(field.field_type))
            })
        })
    }));
    let set = create_memo(clone!(value disabled headings -> move || {
        headings.get() && value.with(Option::is_some) && !disabled.get()
    }));
    let control_id = id.clone();
    let clear = clone!(field on_change -> move || {
        if let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id)) {
            on_change.call(DatabaseValueChange {
                field_id,
                value: None,
                continuous: false,
            });
        }
    });
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Show condition={headings}>
                    <Caption content={heading} />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={set}>
                    <IconButton
                        glyph={ICON_CLEAR.to_owned()}
                        label="Clear"
                        @test_id={format!("{id}.clear")}
                        on_click={clear}
                    />
                </Show>
            </List>
            <DatabaseValueControl
                field={field}
                value={value}
                labels={labels}
                disabled={disabled}
                id={control_id}
                on_change={forward(on_change)}
                on_pick={forward(on_pick)}
                on_submit={forward(on_submit)}
            />
        </List>
    }
}

#[component]
fn DatabaseValueControl(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    labels: Memo<ValueLabels>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
    on_pick: Callback<DatabaseBlockPickRequest>,
    on_submit: Callback<Uuid>,
) -> NodeId {
    let kind = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map(|field| field.field_type))
    }));
    view! {
        <List spacing=0.0>
            <Dynamic value={kind}>
                {move |kind: Option<DatabaseFieldType>| {
                    let field = field.clone();
                    let value = value.clone();
                    let labels = labels.clone();
                    let disabled = disabled.clone();
                    let test_id = id.clone();
                    let on_change = forward(on_change.clone());
                    let on_pick = forward(on_pick.clone());
                    let on_submit = forward(on_submit.clone());
                    match kind {
                        None => view! {
                            <Spacer />
                        },
                        Some(DatabaseFieldType::String) => view! {
                            <StringValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                                on_submit={on_submit}
                            />
                        },
                        Some(DatabaseFieldType::Number) => view! {
                            <NumberValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(DatabaseFieldType::Enum) => view! {
                            <EnumValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(DatabaseFieldType::Boolean) => view! {
                            <BooleanValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(DatabaseFieldType::Color) => view! {
                            <ColorValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(DatabaseFieldType::Datetime) => view! {
                            <DatetimeValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(DatabaseFieldType::Block) => view! {
                            <BlockValue
                                field={field}
                                value={value}
                                labels={labels}
                                disabled={disabled}
                                id={test_id}
                                on_pick={on_pick}
                            />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn StringValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
    on_submit: Callback<Uuid>,
) -> NodeId {
    let text = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::String(text)) => text,
        _ => String::new(),
    }));
    let submitted = clone!(field -> move |_: String| {
        if let Some(id) = field.with_untracked(|field| field.as_ref().map(|field| field.id)) {
            on_submit.call(id);
        }
    });
    let edited = changer(field, on_change, true, |typed: String| {
        Some(DatabaseValue::String(typed))
    });
    view! {
        <TextInput
            value={text}
            placeholder="Empty"
            disabled={disabled}
            @test_id={id}
            on_change={edited}
            on_submit={submitted}
        />
    }
}

#[component]
fn NumberValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let options = create_memo(clone!(field -> move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.number_options)
                .unwrap_or_default()
        })
    }));
    let number = create_memo(clone!(value options -> move || match value.get() {
        Some(DatabaseValue::Number(number)) => number,
        _ => initial_number(options.get()),
    }));
    let bounds = options.get_untracked();
    let edited = changer(field, on_change, true, |typed: f64| {
        Some(DatabaseValue::Number(typed))
    });
    view! {
        <NumberInput
            value={number}
            min={bounds.minimum.unwrap_or(f64::NEG_INFINITY)}
            max={bounds.maximum.unwrap_or(f64::INFINITY)}
            drag={number_drag(bounds)}
            disabled={disabled}
            @test_id={id}
            on_change={edited}
        />
    }
}

#[component]
fn EnumValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let options = create_memo(clone!(field -> move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.enum_options.clone())
                .unwrap_or_default()
        })
    }));
    let keys = create_memo(clone!(options -> move || {
        options.with(|options| options.iter().map(|option| option.id).collect::<Vec<Uuid>>())
    }));
    let selected = create_memo(clone!(options value -> move || {
        let chosen = match value.get() {
            Some(DatabaseValue::Enum(id)) => id,
            _ => return None,
        };
        options.with(|options| options.iter().position(|option| option.id == chosen))
    }));
    let names = options.clone();
    let picked = clone!(options field on_change -> move |index: Option<usize>| {
        let (Some(index), Some(field_id)) = (
            index,
            field.with_untracked(|field| field.as_ref().map(|field| field.id)),
        ) else {
            return;
        };
        let Some(option) = options.with_untracked(|options| options.get(index).cloned()) else {
            return;
        };
        on_change.call(DatabaseValueChange {
            field_id,
            value: Some(DatabaseValue::Enum(option.id)),
            continuous: false,
        });
    });
    view! {
        <Select
            options={view! {
                <ForEach keys={keys}>
                    {move |id: Uuid| {
                        let label = create_memo(clone!(names -> move || {
                            names.with(|options| {
                                options
                                    .iter()
                                    .find(|option| option.id == id)
                                    .map_or_else(String::new, |option| option.name.clone())
                            })
                        }));
                        view! {
                            <ChoiceOption label={label} />
                        }
                    }}
                </ForEach>
            }}
            selected={selected}
            label="Value"
            disabled={disabled}
            @test_id={id}
            on_change={picked}
        />
    }
}

#[component]
fn BooleanValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let checked = create_memo(clone!(value -> move || {
        matches!(value.get(), Some(DatabaseValue::Boolean(true)))
    }));
    let toggled = changer(field, on_change, false, |checked: bool| {
        Some(DatabaseValue::Boolean(checked))
    });
    view! {
        <Checkbox
            label="Checked"
            checked={checked}
            disabled={disabled}
            @test_id={id}
            on_change={toggled}
        />
    }
}

#[component]
fn ColorValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let color = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::Color(color)) => color_of(color),
        _ => Color32::WHITE,
    }));
    let edited = changer(field, on_change, true, |picked: Color32| {
        let [red, green, blue, alpha] = picked.to_array();
        Some(DatabaseValue::Color(DatabaseColor {
            red,
            green,
            blue,
            alpha,
        }))
    });
    view! {
        <ColorInput
            value={color}
            label="Color"
            disabled={disabled}
            @test_id={id}
            on_change={edited}
        />
    }
}

#[component]
fn DatetimeValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let seconds = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::Datetime(seconds)) => Some(seconds),
        _ => None,
    }));
    let unset = create_memo(clone!(seconds -> move || seconds.get().is_none()));
    let known = create_memo(clone!(seconds -> move || seconds.get().is_some()));
    let parts = create_memo(clone!(seconds -> move || {
        DateTimeFields::from_unix(seconds.get().unwrap_or_default())
    }));
    let edited = changer(
        field.clone(),
        on_change.clone(),
        true,
        |parts: DateTimeFields| Some(DatabaseValue::Datetime(parts.to_unix())),
    );
    let now = changer(field, on_change, false, |(): ()| {
        Some(DatabaseValue::Datetime(current_utc_minute()))
    });
    let set = move || now(());
    let set_disabled = disabled.clone();
    view! {
        <List spacing=SPACING>
            <Show condition={unset}>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Button
                        label="Set"
                        variant=ButtonVariant::Secondary
                        disabled={set_disabled}
                        @test_id={format!("{id}.set")}
                        on_click={set}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </Show>
            <Show condition={known}>
                <DateTimeRow value={parts} disabled={disabled} on_change={edited} />
            </Show>
        </List>
    }
}

#[component]
fn BlockValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    labels: Memo<ValueLabels>,
    disabled: Memo<bool>,
    id: String,
    on_pick: Callback<DatabaseBlockPickRequest>,
) -> NodeId {
    let label = create_memo(clone!(value labels -> move || match value.get() {
        Some(DatabaseValue::Block(reference)) => {
            labels.with(|labels| block_reference_text(&reference, labels))
        }
        _ => "Choose block".to_owned(),
    }));
    let choose = clone!(field on_pick -> move || {
        let Some(field) = field.get_untracked() else {
            return;
        };
        on_pick.call(DatabaseBlockPickRequest {
            field_id: field.id,
            block_type: field.block_options.block_type,
        });
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={disabled}
                @test_id={id}
                on_click={choose}
            />
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
pub fn DatabaseValueSummary(text: Prop<String>, aligned: Prop<bool>) -> NodeId {
    let aligned = create_memo(move || aligned.get());
    let align = create_memo(clone!(aligned -> move || match aligned.get() {
        true => beui::TextAlign::End,
        false => beui::TextAlign::Start,
    }));
    view! {
        <Frame padding_horizontal=SPACING>
            <Body content={text} align={align} />
        </Frame>
    }
}

pub fn color_of(color: DatabaseColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha)
}

pub fn current_utc_minute() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    seconds - seconds.rem_euclid(60)
}

fn number_drag(options: DatabaseNumberOptions) -> NumberDrag {
    match options.scale {
        DatabaseNumberScale::Linear => NumberDrag::Linear {
            speed: options.effective_step(),
        },
        DatabaseNumberScale::Logarithmic => NumberDrag::Logarithmic {
            factor: options.effective_step(),
        },
    }
}

fn initial_number(options: DatabaseNumberOptions) -> f64 {
    match options.scale {
        DatabaseNumberScale::Linear => 0.0,
        DatabaseNumberScale::Logarithmic => options.minimum.unwrap_or(1.0),
    }
}

fn field_of(fields: Memo<Vec<DatabaseField>>, id: Uuid) -> Memo<Option<DatabaseField>> {
    create_memo(move || fields.with(|fields| fields.iter().find(|field| field.id == id).cloned()))
}

fn value_of(values: Memo<RowValues>, id: Uuid) -> Memo<Option<DatabaseValue>> {
    create_memo(move || values.with(|values| values.get(&id).cloned()))
}

fn changer<T: 'static>(
    field: Memo<Option<DatabaseField>>,
    on_change: Callback<DatabaseValueChange>,
    continuous: bool,
    make: impl Fn(T) -> Option<DatabaseValue> + 'static,
) -> impl Fn(T) + 'static {
    move |typed| {
        let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id))
        else {
            return;
        };
        let Some(value) = make(typed) else {
            return;
        };
        on_change.call(DatabaseValueChange {
            field_id,
            value: Some(value),
            continuous,
        });
    }
}

fn forward<T: 'static>(callback: Callback<T>) -> impl Fn(T) + 'static {
    move |value| callback.call(value)
}
