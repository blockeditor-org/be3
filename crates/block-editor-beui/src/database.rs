use std::collections::BTreeMap;

use be_block::database::{
    DatabaseColor, DatabaseContent, DatabaseValue, Inverse, RowRef, inverses,
};
use be_block::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale,
    DatabaseSchemaContent,
};
use beui::datetime::DateTime;
use beui::icons::ICON_CLEAR;
use beui::reactive::{
    Align, Callback, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, Prop, ReadSignal,
    Show, Spacer, clone, component, create_effect, create_memo, create_signal, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, ColorInput, DateTimeField, IconButton,
    NumberDrag, NumberInput, Select, TextInput,
};
use beui::unstyled::ChoiceOption;
use beui::{Color32, NodeId};
pub use block_ui::database::ValueLabels;
use block_ui::database::{
    DatabaseBlockPickRequest, DatabaseValueChange, block_reference_text, field_type_label,
};
use uuid::Uuid;

const SPACING: f32 = 6.0;
const FIELD_SPACING: f32 = 12.0;

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
                    <Caption content={heading.clone()} />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={set}>
                    <IconButton
                        glyph={ICON_CLEAR.to_owned()}
                        label="Clear"
                        @test_id={format!("{id}.clear")}
                        on_click={clear.clone()}
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
        field.with(|field| {
            field.as_ref().map(|field| match field.field_type {
                DatabaseFieldType::Relation if field.relation_options.multiple => None,
                other => Some(other),
            })
        })
    }));
    view! {
        <List spacing=0.0>
            <Dynamic value={kind}>
                {move |kind: Option<Option<DatabaseFieldType>>| {
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
                        Some(Some(DatabaseFieldType::String)) => view! {
                            <StringValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                                on_submit={on_submit}
                            />
                        },
                        Some(Some(DatabaseFieldType::Number)) => view! {
                            <NumberValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Enum)) => view! {
                            <EnumValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Boolean)) => view! {
                            <BooleanValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Color)) => view! {
                            <ColorValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Datetime)) => view! {
                            <DatetimeValue
                                field={field}
                                value={value}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(None) => view! {
                            <LinksValue
                                field={field}
                                value={value}
                                labels={labels}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Relation)) => view! {
                            <RelationValue
                                field={field}
                                value={value}
                                labels={labels}
                                disabled={disabled}
                                id={test_id}
                                on_change={on_change}
                            />
                        },
                        Some(Some(DatabaseFieldType::Block)) => view! {
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
    let moment = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::Datetime(seconds)) => Some(DateTime::from_unix(seconds)),
        _ => None,
    }));
    let label = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map(|field| field.name.clone()).unwrap_or_default())
    }));
    let edited = move |next: Option<DateTime>| {
        let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id))
        else {
            return;
        };
        on_change.call(DatabaseValueChange {
            field_id,
            value: next.map(|next| DatabaseValue::Datetime(next.to_unix())),
            continuous: true,
        });
    };
    view! {
        <DateTimeField value={moment} label disabled @test_id={id} on_change={edited} />
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
            labels.with(|labels| block_reference_text(&reference, &labels.blocks))
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
fn RelationValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    labels: Memo<ValueLabels>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let target = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().and_then(|field| field.relation_options.database))
    }));
    let choices = create_memo(clone!(target labels -> move || {
        let Some(database) = target.get() else {
            return Vec::new();
        };
        labels.with(|labels| labels.rows_of(database))
    }));
    let keys = create_memo(clone!(choices -> move || {
        choices.with(|choices| choices.iter().map(|(link, _)| *link).collect::<Vec<RowRef>>())
    }));
    let selected = create_memo(clone!(choices value -> move || {
        let Some(DatabaseValue::Relation(chosen)) = value.get() else {
            return None;
        };
        choices.with(|choices| choices.iter().position(|(link, _)| *link == chosen))
    }));
    let unlinked = create_memo(clone!(target -> move || target.get().is_none()));
    let names = choices.clone();
    let picked = clone!(choices field on_change -> move |index: Option<usize>| {
        let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id))
        else {
            return;
        };
        let link = index.and_then(|index| {
            choices.with_untracked(|choices| choices.get(index).map(|(link, _)| *link))
        });
        on_change.call(DatabaseValueChange {
            field_id,
            value: link.map(DatabaseValue::Relation),
            continuous: false,
        });
    });
    view! {
        <List spacing=SPACING>
            <Show condition={unlinked}>
                <Caption content="Choose the database this column links to." wrap=true />
            </Show>
            <Select
                options={view! {
                    <ForEach keys={keys}>
                        {move |link: RowRef| {
                            let label = create_memo(clone!(names -> move || {
                                names.with(|choices| {
                                    choices
                                        .iter()
                                        .find(|(held, _)| *held == link)
                                        .map_or_else(String::new, |(_, label)| label.clone())
                                })
                            }));
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }}
                    </ForEach>
                }}
                selected={selected}
                label="Row"
                disabled={disabled}
                @test_id={id}
                on_change={picked}
            />
        </List>
    }
}

#[component]
fn LinksValue(
    field: Memo<Option<DatabaseField>>,
    value: Memo<Option<DatabaseValue>>,
    labels: Memo<ValueLabels>,
    disabled: Memo<bool>,
    id: String,
    on_change: Callback<DatabaseValueChange>,
) -> NodeId {
    let linked = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::Links(links)) => links,
        Some(DatabaseValue::Relation(link)) => vec![link],
        _ => Vec::new(),
    }));
    let target = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().and_then(|field| field.relation_options.database))
    }));
    let addable = create_memo(clone!(target labels linked -> move || {
        let Some(database) = target.get() else {
            return Vec::new();
        };
        let held = linked.get();
        labels.with(|labels| {
            labels
                .rows_of(database)
                .into_iter()
                .filter(|(link, _)| !held.contains(link))
                .collect::<Vec<_>>()
        })
    }));
    let add_keys = create_memo(clone!(addable -> move || {
        addable.with(|rows| rows.iter().map(|(link, _)| *link).collect::<Vec<RowRef>>())
    }));
    let emit = clone!(field on_change -> move |links: Vec<RowRef>| {
        let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id))
        else {
            return;
        };
        on_change.call(DatabaseValueChange {
            field_id,
            value: (!links.is_empty()).then_some(DatabaseValue::Links(links)),
            continuous: false,
        });
    });
    let added = clone!(addable linked emit -> move |index: Option<usize>| {
        let Some(link) = index.and_then(|index| {
            addable.with_untracked(|rows| rows.get(index).map(|(link, _)| *link))
        }) else {
            return;
        };
        let mut links = linked.get_untracked();
        links.push(link);
        emit(links);
    });
    let names = addable.clone();
    let chip_labels = labels.clone();
    let add_id = format!("{id}.add");
    let add_off = disabled.clone();
    view! {
        <List spacing=SPACING>
            <ForEach keys={linked.clone()}>
                {move |link: RowRef| {
                    let disabled = disabled.clone();
                    let remove_id = format!("{id}.remove.{}", link.row);
                    let label = create_memo(clone!(chip_labels -> move || {
                        chip_labels.with(|labels| labels.row(&link))
                    }));
                    let remove = clone!(linked emit -> move || {
                        let links = linked
                            .get_untracked()
                            .into_iter()
                            .filter(|held| *held != link)
                            .collect();
                        emit(links);
                    });
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Body @sizing=ItemSize::Percent(100.0) content={label} />
                            <IconButton
                                glyph={ICON_CLEAR.to_owned()}
                                label="Remove link"
                                disabled={disabled}
                                @test_id={remove_id}
                                on_click={remove}
                            />
                        </List>
                    }
                }}
            </ForEach>
            <Select
                options={view! {
                    <ForEach keys={add_keys}>
                        {move |link: RowRef| {
                            let label = create_memo(clone!(names -> move || {
                                names.with(|rows| {
                                    rows.iter()
                                        .find(|(held, _)| *held == link)
                                        .map_or_else(String::new, |(_, label)| label.clone())
                                })
                            }));
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }}
                    </ForEach>
                }}
                selected={None}
                label="Add a link"
                disabled={add_off}
                @test_id={add_id}
                on_change={added}
            />
        </List>
    }
}

pub fn watch_inverses(editor: &crate::Editor, database: Memo<Option<Uuid>>) -> Memo<Vec<Inverse>> {
    let (found, set_found) = create_signal(Vec::<Inverse>::new());
    let editor = editor.clone();
    let watched: std::cell::RefCell<Option<(Uuid, crate::BlockList)>> =
        std::cell::RefCell::new(None);
    create_effect(move || {
        let Some(target) = database.get() else {
            set_found.set(Vec::new());
            return;
        };
        let mut held = watched.borrow_mut();
        if held.as_ref().is_none_or(|(id, _)| *id != target) {
            *held = Some((
                target,
                editor.blocks().watch(crate::BlockQuery::Backrefs(target)),
            ));
        }
        let mut sources: Vec<Uuid> = held
            .as_ref()
            .map(|(_, list)| list.read())
            .unwrap_or_default()
            .into_iter()
            .filter(|info| {
                info.block_type == <DatabaseSchemaContent as be_block::BlockContent>::CONTENT_TYPE
            })
            .filter_map(|info| match info.parent {
                crate::BlockParent::Block(parent) => Some(parent),
                _ => None,
            })
            .collect();
        drop(held);
        sources.sort();
        sources.dedup();
        let watched_sources: Vec<ReadSignal<Vec<Inverse>>> = sources
            .into_iter()
            .map(|source| source_inverses(&editor, target, source))
            .collect();
        let set_found = set_found.clone();
        create_effect(move || {
            set_found.set(
                watched_sources
                    .iter()
                    .flat_map(|inverses| inverses.get())
                    .collect(),
            );
        });
    });
    create_memo(move || found.get())
}

fn source_inverses(editor: &crate::Editor, target: Uuid, source: Uuid) -> ReadSignal<Vec<Inverse>> {
    let content = editor.content_of::<DatabaseContent>(source);
    let schema_id = content.project(|content| content.root().schema);
    let (found, set_found) = create_signal(Vec::new());
    let editor = editor.clone();
    create_effect(move || {
        let Some(schema) = schema_id.get() else {
            set_found.set(Vec::new());
            return;
        };
        let schema = editor
            .content_of::<DatabaseSchemaContent>(schema)
            .project(|schema| schema.root());
        let database = content.project(|content| content.root());
        let set_found = set_found.clone();
        create_effect(move || {
            set_found.set(inverses(target, source, &database.get(), &schema.get()));
        });
    });
    found
}

pub fn relation_targets(fields: &[DatabaseField]) -> Vec<Uuid> {
    let mut targets: Vec<Uuid> = fields
        .iter()
        .filter(|field| field.field_type == DatabaseFieldType::Relation)
        .filter_map(|field| field.relation_options.database)
        .collect();
    targets.sort();
    targets.dedup();
    targets
}

pub fn watch_row_labels(
    editor: &crate::Editor,
    databases: Memo<Vec<Uuid>>,
) -> Memo<BTreeMap<RowRef, String>> {
    let (labels, set_labels) = create_signal(BTreeMap::new());
    let editor = editor.clone();
    create_effect(move || {
        let watched: Vec<ReadSignal<Vec<(RowRef, String)>>> = databases
            .get()
            .into_iter()
            .map(|database| row_titles(&editor, database))
            .collect();
        let set_labels = set_labels.clone();
        create_effect(move || {
            set_labels.set(
                watched
                    .iter()
                    .flat_map(|titles| titles.get())
                    .collect::<BTreeMap<RowRef, String>>(),
            );
        });
    });
    create_memo(move || labels.get())
}

fn row_titles(editor: &crate::Editor, database: Uuid) -> ReadSignal<Vec<(RowRef, String)>> {
    let content = editor.content_of::<DatabaseContent>(database);
    let schema_id = content.project(|content| content.root().schema);
    let (titles, set_titles) = create_signal(Vec::new());
    let editor = editor.clone();
    create_effect(move || {
        let title_field = schema_id.get().map(|schema| {
            editor
                .content_of::<DatabaseSchemaContent>(schema)
                .project(|schema| schema.root().title_field())
        });
        let content = content.clone();
        let set_titles = set_titles.clone();
        create_effect(move || {
            let title = title_field.as_ref().and_then(ReadSignal::get);
            let projected = content.project(move |content| {
                let root = content.root();
                root.rows
                    .iter()
                    .map(|row| {
                        let link = RowRef {
                            database,
                            row: row.id.as_uuid(),
                        };
                        (link, root.title(row.id, title))
                    })
                    .collect::<Vec<_>>()
            });
            let set_titles = set_titles.clone();
            create_effect(move || set_titles.set(projected.get()));
        });
    });
    titles
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

pub fn create_database(creation: &crate::Creation) -> Uuid {
    use crate::BlockParent;
    use be_block::database::{Database, DatabaseContent};
    use be_block::database_schema::{DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent};

    let blocks = creation.blocks();
    let mut fields = DatabaseSchemaContent::default();
    fields.apply(&DatabaseSchema::add_field("Name", DatabaseFieldType::String).1);
    let schema = blocks.create(&fields, BlockParent::Detached);
    let database = blocks.create(
        &DatabaseContent::new(&Database::with_schema(schema)),
        BlockParent::Detached,
    );
    blocks.set_parent(schema, BlockParent::Block(database));
    database
}
