use beui::Overflow;
use std::collections::BTreeMap;

use beui::datetime::DateTime;
use beui::icons::ICON_X;
use beui::reactive::*;
use beui::styled::*;
use beui::{Color32, NodeId};
pub use block_ui::database::ValueLabels;
use block_ui::database::{DatabaseValueChange, block_reference_text, field_type_label};
use database_types::database::{
    Database, DatabaseColor, DatabaseContent, DatabaseValue, Inverse, RowRef, inverses_of, short_id,
};
use database_types::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale,
    DatabaseSchemaContent,
};
use uuid::Uuid;

mod choice;

use beui::{Align, Direction, ItemSize};
pub use choice::{
    ChoiceColors, ChoiceMark, choice_color_label, choice_glyph, choice_icon_label,
    use_choice_colors,
};

const SPACING: Space = Space::Sm;
const FIELD_SPACING: Space = Space::Lg;

pub type RowValues = BTreeMap<Uuid, DatabaseValue>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DatabaseBlockPickRequest {
    pub field_id: Uuid,
    pub block_type: Option<Uuid>,
    pub anchor: crate::PickAnchor,
}

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
    let keys = create_memo(move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    });
    view! {
        <List spacing=FIELD_SPACING>
            <ForEach keys={keys}>|id: Uuid| {
                let field = field_of(fields, id);
                let value = value_of(values, id);
                let test_id = format!("{}.field.{id}", prefix.get_untracked());
                view! {
                    <DatabaseValueRow
                        field={field}
                        value={value}
                        labels={labels}
                        headings={headings}
                        disabled={disabled}
                        id={test_id}
                        on_change={forward(on_change)}
                        on_pick={forward(on_pick)}
                        on_submit={forward(on_submit)}
                    />
                }
            }</ForEach>
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
    let heading = create_memo(move || {
        field.with(|field| {
            field.as_ref().map_or_else(String::new, |field| {
                format!("{} ({})", field.name, field_type_label(field.field_type))
            })
        })
    });
    let set = create_memo(move || headings.get() && value.with(Option::is_some) && !disabled.get());
    let control_id = id.clone();
    let clear = move || {
        if let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id)) {
            on_change.call(DatabaseValueChange {
                field_id,
                value: None,
                continuous: false,
            });
        }
    };
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Show condition={headings}>
                    <Text style=TextStyle::CAPTION>{heading}</Text>
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={set}>
                    <Button
                        icon_only=true
                        glyph=ICON_X
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
    let kind = create_memo(move || {
        field.with(|field| {
            field.as_ref().map(|field| match field.field_type {
                DatabaseFieldType::Relation if field.relation_options.multiple => None,
                other => Some(other),
            })
        })
    });
    view! {
        <List spacing=Space::None>
            <Dynamic value={kind}>|kind: Option<Option<DatabaseFieldType>>| {
                let test_id = id.clone();
                let on_change = forward(on_change);
                let on_pick = forward(on_pick);
                let on_submit = forward(on_submit);
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
            }</Dynamic>
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
    let text = create_memo(move || match value.get() {
        Some(DatabaseValue::String(text)) => text,
        _ => String::new(),
    });
    let submitted = move |_: String| {
        if let Some(id) = field.with_untracked(|field| field.as_ref().map(|field| field.id)) {
            on_submit.call(id);
        }
    };
    let edited = changer(field, on_change, true, |typed: String| {
        Some(DatabaseValue::String(typed))
    });
    view! {
        <TextInput
            value={Binding::new(text, edited)}
            placeholder="Empty"
            disabled={disabled}
            @test_id={id}
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
    let options = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.number_options)
                .unwrap_or_default()
        })
    });
    let number = create_memo(move || match value.get() {
        Some(DatabaseValue::Number(number)) => number,
        _ => initial_number(options.get()),
    });
    let bounds = options.get_untracked();
    let edited = changer(field, on_change, true, |typed: f64| {
        Some(DatabaseValue::Number(typed))
    });
    view! {
        <NumberInput
            value={Binding::new(number, edited)}
            min={bounds.minimum.unwrap_or(f64::NEG_INFINITY)}
            max={bounds.maximum.unwrap_or(f64::INFINITY)}
            drag={number_drag(bounds)}
            disabled={disabled}
            @test_id={id}
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
    let options = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.enum_options.clone())
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        options.with(|options| {
            options
                .iter()
                .map(|option| option.id)
                .collect::<Vec<Uuid>>()
        })
    });
    let selected = create_memo(move || {
        let chosen = match value.get() {
            Some(DatabaseValue::Enum(id)) => id,
            _ => return None,
        };
        options.with(|options| options.iter().position(|option| option.id == chosen))
    });
    let picked = move |index: Option<usize>| {
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
    };
    view! {
        <Select
            options={view! {
                <ForEach keys={keys}>|id: Uuid| {
                    let label = create_memo(move || {
                        options.with(|options| {
                            options
                                .iter()
                                .find(|option| option.id == id)
                                .map_or_else(String::new, |option| option.name.clone())
                        })
                    });
                    view! {
                        <ChoiceOption label={label} />
                    }
                }</ForEach>
            }}
            selected={Binding::new(selected, picked)}
            label="Value"
            disabled={disabled}
            @test_id={id}
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
    let checked = create_memo(move || matches!(value.get(), Some(DatabaseValue::Boolean(true))));
    let toggled = changer(field, on_change, false, |checked: bool| {
        Some(DatabaseValue::Boolean(checked))
    });
    view! {
        <Checkbox
            label="Checked"
            checked={Binding::new(checked, toggled)}
            disabled={disabled}
            @test_id={id}
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
    let color = create_memo(move || match value.get() {
        Some(DatabaseValue::Color(color)) => color_of(color),
        _ => Color32::WHITE,
    });
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
            value={Binding::new(color, edited)}
            label="Color"
            disabled={disabled}
            @test_id={id}
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
    let moment = create_memo(move || match value.get() {
        Some(DatabaseValue::Datetime(seconds)) => Some(DateTime::from_unix(seconds)),
        _ => None,
    });
    let label = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.name.clone())
                .unwrap_or_default()
        })
    });
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
        <DateTimeField value={Binding::new(moment, edited)} label disabled @test_id={id} />
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
    let label = create_memo(move || match value.get() {
        Some(DatabaseValue::Block(reference)) => {
            labels.with(|labels| block_reference_text(&reference, &labels.blocks))
        }
        _ => "Choose block".to_owned(),
    });
    let button = NodeRef::new();
    let choose = move || {
        let Some(field) = field.get_untracked() else {
            return;
        };
        on_pick.call(DatabaseBlockPickRequest {
            field_id: field.id,
            block_type: field.block_options.block_type,
            anchor: crate::PickAnchor::Node(button),
        });
    };
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Button
                @node_ref={&button}
                label={label}
                variant=ButtonVariant::Secondary
                disabled={disabled}
                @test_id={id}
                @sizing={ItemSize::Intrinsic.shrink(1.0)}
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
    let target = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .and_then(|field| field.relation_options.database)
        })
    });
    let choices = create_memo(move || {
        let Some(database) = target.get() else {
            return Vec::new();
        };
        labels.with(|labels| labels.rows_of(database))
    });
    let keys = create_memo(move || {
        choices.with(|choices| {
            choices
                .iter()
                .map(|(link, _)| *link)
                .collect::<Vec<RowRef>>()
        })
    });
    let selected = create_memo(move || {
        let Some(DatabaseValue::Relation(chosen)) = value.get() else {
            return None;
        };
        choices.with(|choices| choices.iter().position(|(link, _)| *link == chosen))
    });
    let unlinked = create_memo(move || target.get().is_none());
    let picked = move |index: Option<usize>| {
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
    };
    view! {
        <List spacing=SPACING>
            <Show condition={unlinked}>
                <Text style=TextStyle::CAPTION>"Choose the database this column links to."</Text>
            </Show>
            <Select
                options={view! {
                    <ForEach keys={keys}>|link: RowRef| {
                        let label = create_memo(move || {
                            choices.with(|choices| {
                                choices
                                    .iter()
                                    .find(|(held, _)| *held == link)
                                    .map_or_else(String::new, |(_, label)| label.clone())
                            })
                        });
                        view! {
                            <ChoiceOption label={label} />
                        }
                    }</ForEach>
                }}
                selected={Binding::new(selected, picked)}
                label="Row"
                disabled={disabled}
                @test_id={id}
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
    let linked = create_memo(move || match value.get() {
        Some(DatabaseValue::Links(links)) => links,
        Some(DatabaseValue::Relation(link)) => vec![link],
        _ => Vec::new(),
    });
    let target = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .and_then(|field| field.relation_options.database)
        })
    });
    let addable = create_memo(move || {
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
    });
    let add_keys = create_memo(move || {
        addable.with(|rows| rows.iter().map(|(link, _)| *link).collect::<Vec<RowRef>>())
    });
    let emit = move |links: Vec<RowRef>| {
        let Some(field_id) = field.with_untracked(|field| field.as_ref().map(|field| field.id))
        else {
            return;
        };
        on_change.call(DatabaseValueChange {
            field_id,
            value: (!links.is_empty()).then_some(DatabaseValue::Links(links)),
            continuous: false,
        });
    };
    let added = move |index: Option<usize>| {
        let Some(link) = index.and_then(|index| {
            addable.with_untracked(|rows| rows.get(index).map(|(link, _)| *link))
        }) else {
            return;
        };
        let mut links = linked.get_untracked();
        links.push(link);
        emit(links);
    };
    let add_id = format!("{id}.add");
    view! {
        <List spacing=SPACING>
            <ForEach keys={linked}>|link: RowRef| {
                let remove_id = format!("{id}.remove.{}", link.row);
                let label = create_memo(move || {
                    labels.with(|labels| labels.row(&link))
                });
                let remove = move || {
                    let links = linked
                        .get_untracked()
                        .into_iter()
                        .filter(|held| *held != link)
                        .collect();
                    emit(links);
                };
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>
                            {label}
                        </Text>
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Remove link"
                            disabled={disabled}
                            @test_id={remove_id}
                            on_click={remove}
                        />
                    </List>
                }
            }</ForEach>
            <Select
                options={view! {
                    <ForEach keys={add_keys}>|link: RowRef| {
                        let label = create_memo(move || {
                            addable.with(|rows| {
                                rows.iter()
                                    .find(|(held, _)| *held == link)
                                    .map_or_else(String::new, |(_, label)| label.clone())
                            })
                        });
                        view! {
                            <ChoiceOption label={label} />
                        }
                    }</ForEach>
                }}
                selected={Binding::new(None, added)}
                label="Add a link"
                disabled={disabled}
                @test_id={add_id}
            />
        </List>
    }
}

pub fn watch_inverses(
    editor: crate::Editor,
    database: Memo<Option<Uuid>>,
) -> Memo<Option<Vec<Inverse>>> {
    let found = create_signal(None::<Vec<Inverse>>);
    let watched: std::cell::RefCell<Option<(Uuid, crate::BlockList)>> =
        std::cell::RefCell::new(None);
    create_binding(move || {
        let Some(target) = database.get() else {
            return;
        };
        let mut held = watched.borrow_mut();
        if held.as_ref().is_none_or(|(id, _)| *id != target) {
            *held = Some((
                target,
                editor.blocks().watch(crate::BlockQuery::Backrefs(target)),
            ));
        }
        let Some((_, backrefs)) = held.as_ref().filter(|(_, list)| list.read().is_ok()) else {
            return;
        };
        let mut sources: Vec<Uuid> = backrefs
            .read()
            .unwrap_or_default()
            .into_iter()
            .filter(|info| {
                info.block_type == <DatabaseSchemaContent as be_block::BlockContent>::CONTENT_TYPE
                    && info.access.can_view()
            })
            .filter_map(|info| match info.parent {
                Some(crate::BlockParent::Block(parent)) => Some(parent),
                _ => None,
            })
            .collect();
        drop(held);
        sources.sort();
        sources.dedup();
        let watched_sources: Vec<ReadSignal<Option<Vec<Inverse>>>> = sources
            .into_iter()
            .map(|source| source_inverses(editor, target, source))
            .collect();
        create_binding(move || {
            let mut all = Vec::new();
            for inverses in &watched_sources {
                let Some(inverses) = inverses.get() else {
                    return;
                };
                all.extend(inverses);
            }
            found.set(Some(all));
        });
    });
    wait_for(move || {
        if database.get().is_some() && found.with(Option::is_none) {
            Err(Pending::Loading)
        } else {
            Ok(())
        }
    });
    create_memo(move || found.get())
}

fn settled<C>(
    editor: crate::Editor,
    block: Uuid,
    content: crate::ContentProjection<C>,
) -> Memo<bool>
where
    C: be_block::LiveEdit + Clone + Default,
{
    let found = editor.blocks().watch(crate::BlockQuery::Block(block));
    create_memo(move || {
        content.ready().is_ok()
            || found
                .read()
                .is_ok_and(|found| found.first().is_none_or(|info| !info.access.can_view()))
    })
}

fn source_inverses(
    editor: crate::Editor,
    target: Uuid,
    source: Uuid,
) -> ReadSignal<Option<Vec<Inverse>>> {
    let content = editor.content_of::<DatabaseContent>(source);
    let content_settled = settled(editor, source, content);
    let schema_id = content.field(be_block::ObjectId::ROOT, Database::SCHEMA);
    let found = create_signal(None);
    create_binding(move || {
        if !content_settled.get() {
            return;
        }
        let Some(schema) = schema_id.get() else {
            found.set(Some(Vec::new()));
            return;
        };
        let schema_content = editor.content_of::<DatabaseSchemaContent>(schema);
        let schema_settled = settled(editor, schema, schema_content);
        let schema = schema_content.project(|schema| schema.root());
        let rows = content.objects(be_block::ObjectId::ROOT, Database::ROWS);
        create_binding(move || {
            if schema_settled.get() {
                let all = rows.with(|rows| {
                    schema.with(|schema| {
                        inverses_of(
                            target,
                            source,
                            rows.iter().map(|(id, row)| (*id, row.as_ref())),
                            schema,
                        )
                    })
                });
                found.set(Some(all));
            }
        });
    });
    found.read_only()
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

type RowTitles = Vec<(RowRef, String)>;

#[derive(Clone, Copy)]
pub struct RowLabels {
    pub labels: Memo<BTreeMap<RowRef, String>>,
    pub unreachable: Memo<std::collections::BTreeSet<Uuid>>,
}

pub fn watch_row_labels(editor: crate::Editor, databases: Memo<Vec<Uuid>>) -> RowLabels {
    let found = create_signal(None::<(Vec<Uuid>, BTreeMap<RowRef, String>)>);
    create_binding(move || {
        let wanted = databases.get();
        let watched: Vec<ReadSignal<Option<RowTitles>>> = wanted
            .iter()
            .map(|database| row_titles(editor, *database))
            .collect();
        create_binding(move || {
            let mut all = BTreeMap::new();
            for titles in &watched {
                let Some(titles) = titles.get() else {
                    return;
                };
                all.extend(titles);
            }
            found.set(Some((wanted.clone(), all)));
        });
    });
    let blocks = editor.blocks();
    let unreachable = create_memo(move || {
        databases.with(|databases| {
            databases
                .iter()
                .copied()
                .filter(|database| blocks.info(*database).is_some_and(|info| info.no_access()))
                .collect()
        })
    });
    let loaded = create_memo(move || {
        let wanted = databases.get();
        found.with(|found| {
            found
                .as_ref()
                .is_some_and(|(covered, _)| *covered == wanted)
        })
    });
    wait_for(move || {
        if loaded.get() {
            Ok(())
        } else {
            Err(Pending::Loading)
        }
    });
    RowLabels {
        unreachable,
        labels: create_memo(move || {
            found.with(|found| {
                found
                    .as_ref()
                    .map(|(_, labels)| labels.clone())
                    .unwrap_or_default()
            })
        }),
    }
}

fn row_titles(editor: crate::Editor, database: Uuid) -> ReadSignal<Option<RowTitles>> {
    let content = editor.content_of::<DatabaseContent>(database);
    let content_settled = settled(editor, database, content);
    let schema_id = content.field(be_block::ObjectId::ROOT, Database::SCHEMA);
    let titles = create_signal(None);
    create_binding(move || {
        if !content_settled.get() {
            return;
        }
        let title_field = schema_id.get().map(|schema| {
            let schema_content = editor.content_of::<DatabaseSchemaContent>(schema);
            let settled = settled(editor, schema, schema_content);
            let title = schema_content.project(|schema| schema.root().title_field());
            (settled, title)
        });
        create_binding(move || {
            let title = match &title_field {
                Some((settled, _)) if !settled.get() => return,
                Some((_, title)) => title.get(),
                None => None,
            };
            let prefix = content.field(be_block::ObjectId::ROOT, Database::PREFIX);
            let rows = content.objects(be_block::ObjectId::ROOT, Database::ROWS);
            create_binding(move || {
                let prefix = prefix.get();
                let projected = rows.with(|rows| {
                    rows.iter()
                        .map(|(id, row)| {
                            let link = RowRef {
                                database,
                                row: id.as_uuid(),
                            };
                            let named =
                                title.and_then(|field| row.value(field)).and_then(|value| {
                                    match value {
                                        DatabaseValue::String(text) if !text.trim().is_empty() => {
                                            Some(text.clone())
                                        }
                                        _ => None,
                                    }
                                });
                            (
                                link,
                                named.unwrap_or_else(|| short_id(&prefix, id.as_uuid())),
                            )
                        })
                        .collect::<Vec<_>>()
                });
                titles.set(Some(projected));
            });
        });
    });
    titles.read_only()
}

#[component]
pub fn DatabaseValueSummary(text: Prop<String>, aligned: Prop<bool>) -> NodeId {
    let aligned = create_memo(move || aligned.get());
    let align = create_memo(move || match aligned.get() {
        true => beui::TextAlign::End,
        false => beui::TextAlign::Start,
    });
    view! {
        <Frame padding={Padding::horizontal(SPACING)}>
            <Text align={align}>{text}</Text>
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
    use database_types::database_schema::DatabaseSchema;

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
