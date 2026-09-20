use std::rc::Rc;

use block_client::blocks::database_schema::{
    DatabaseBlockOptions, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchema, DatabaseSchemaOperation,
};
use block_editor_plugin::beui::icons::{ICON_ADD, ICON_DELETE};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_effect, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Checkbox, Heading, IconButton, NumberInput, Scroll, Select,
    Separator, TextInput, use_theme,
};
use block_editor_plugin::beui::unstyled::ChoiceOption;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockProjection, Editor};
use uuid::Uuid;

use super::field_line_count;

type Schema = Rc<BlockProjection<DatabaseSchema>>;

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const INDENT: f32 = 16.0;
const NAME_WIDTH: f32 = 240.0;
const NUMBER_WIDTH: f32 = 120.0;
const INTRINSIC_WIDTH: f32 = 600.0;
const ROW_HEIGHT: f32 = 40.0;

const FIELD_TYPES: [(DatabaseFieldType, &str, &str); 7] = [
    (DatabaseFieldType::String, "String", "string"),
    (DatabaseFieldType::Number, "Number", "number"),
    (DatabaseFieldType::Enum, "Enum", "enum"),
    (DatabaseFieldType::Block, "Block", "block"),
    (DatabaseFieldType::Boolean, "Boolean", "boolean"),
    (DatabaseFieldType::Color, "Color", "color"),
    (DatabaseFieldType::Datetime, "Datetime", "datetime"),
];

const NUMBER_SCALES: [(DatabaseNumberScale, &str); 2] = [
    (DatabaseNumberScale::Linear, "Linear"),
    (DatabaseNumberScale::Logarithmic, "Logarithmic"),
];

#[component]
pub fn SchemaView(editor: Editor) -> NodeId {
    let schema = editor.block::<DatabaseSchema>();
    let fields = schema.project(|schema| schema.fields().to_vec());
    let rows = create_memo(clone!(fields -> move || fields.get()));
    let keys = create_memo(clone!(rows -> move || {
        rows.with(|rows| rows.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    }));
    let read_only = editor.read_only();

    let sized = editor.clone();
    create_effect(clone!(rows -> move || {
        let (lines, count) = rows.with(|rows| {
            (
                rows.iter().map(field_line_count).sum::<usize>() + 2,
                rows.len(),
            )
        });
        let height = ROW_HEIGHT * lines as f32 + SECTION_SPACING * count as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let add = clone!(schema -> move || {
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: Uuid::new_v4(),
                name: "Field".into(),
                field_type: DatabaseFieldType::String,
                enum_options: Vec::new(),
                number_options: DatabaseNumberOptions::default(),
                block_options: DatabaseBlockOptions::default(),
            },
        });
    });

    let add_disabled = read_only.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SECTION_SPACING>
                <Heading content="Fields" />
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <ForEach keys={keys}>
                            {move |id: Uuid| {
                                let field = create_memo(clone!(rows id -> move || {
                                    rows.with(|rows| {
                                        rows.iter().find(|field| field.id == id).cloned()
                                    })
                                }));
                                view! {
                                    <FieldRow
                                        editor={editor.clone()}
                                        schema={schema.clone()}
                                        id={id}
                                        field={field}
                                        read_only={read_only.clone()}
                                    />
                                }
                            }}
                        </ForEach>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=ROW_SPACING
                        >
                            <Button
                                label="Add field"
                                variant=ButtonVariant::Primary
                                disabled={add_disabled}
                                @test_id={"database-schema.add-field"}
                                on_click={add}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        </List>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn FieldRow(
    editor: Editor,
    schema: Schema,
    id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
) -> NodeId {
    let name = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map_or_else(String::new, |field| field.name.clone()))
    }));
    let kind = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map(|field| field.field_type))
    }));
    let selected = create_memo(clone!(kind -> move || {
        kind.get().and_then(|kind| {
            FIELD_TYPES.iter().position(|(value, _, _)| *value == kind)
        })
    }));
    let is =
        |wanted: DatabaseFieldType| create_memo(clone!(kind -> move || kind.get() == Some(wanted)));
    let renamed = clone!(schema -> move |name: String| {
        schema.operate(DatabaseSchemaOperation::RenameField { field_id: id, name });
    });
    let retyped = clone!(schema -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        schema.operate(DatabaseSchemaOperation::SetFieldType {
            field_id: id,
            field_type: FIELD_TYPES[index].0,
        });
    });
    let remove = clone!(schema -> move || {
        schema.operate(DatabaseSchemaOperation::RemoveField { field_id: id });
    });
    let type_keys = create_memo(|| (0..FIELD_TYPES.len()).collect::<Vec<usize>>());
    let (name_off, type_off, delete_off) =
        (read_only.clone(), read_only.clone(), read_only.clone());
    let (number_off, enum_off, block_off) = (read_only.clone(), read_only.clone(), read_only);
    let (number_schema, enum_schema, block_schema) = (schema.clone(), schema.clone(), schema);
    let (number_field, enum_field, block_field) = (field.clone(), field.clone(), field);
    let (number_kind, enum_kind, block_kind) = (
        is(DatabaseFieldType::Number),
        is(DatabaseFieldType::Enum),
        is(DatabaseFieldType::Block),
    );
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=NAME_WIDTH>
                    <TextInput
                        value={name}
                        label="Field name"
                        disabled={name_off}
                        @test_id={format!("database-schema.name.{id}")}
                        on_change={renamed}
                    />
                </Frame>
                <Select
                    options={view! {
                        <ForEach keys={type_keys}>
                            {|index: usize| view! {
                                <ChoiceOption label={FIELD_TYPES[index].1} />
                            }}
                        </ForEach>
                    }}
                    selected={selected}
                    label="Field type"
                    disabled={type_off}
                    @test_id={format!("database-schema.type.{id}")}
                    on_change={retyped}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Delete field"
                    disabled={delete_off}
                    @test_id={format!("database-schema.delete.{id}")}
                    on_click={remove}
                />
            </List>
            <Show condition={number_kind}>
                <NumberOptions
                    schema={number_schema}
                    id={id}
                    field={number_field}
                    read_only={number_off}
                />
            </Show>
            <Show condition={enum_kind}>
                <EnumOptions schema={enum_schema} id={id} field={enum_field} read_only={enum_off} />
            </Show>
            <Show condition={block_kind}>
                <BlockOptions
                    editor={editor}
                    schema={block_schema}
                    id={id}
                    field={block_field}
                    read_only={block_off}
                />
            </Show>
            <Separator />
        </List>
    }
}

#[component]
fn NumberOptions(
    schema: Schema,
    id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
) -> NodeId {
    let options = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map(|field| field.number_options).unwrap_or_default())
    }));
    let scale = create_memo(clone!(options -> move || {
        NUMBER_SCALES
            .iter()
            .position(|(value, _)| *value == options.get().scale)
    }));
    let rescaled = clone!(schema options -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let mut next = options.get_untracked();
        next.scale = NUMBER_SCALES[index].0;
        schema.operate(DatabaseSchemaOperation::SetNumberOptions {
            field_id: id,
            options: next,
        });
    });
    let scale_keys = create_memo(|| (0..NUMBER_SCALES.len()).collect::<Vec<usize>>());
    let (minimum_off, maximum_off, step_off, scale_off) = (
        read_only.clone(),
        read_only.clone(),
        read_only.clone(),
        read_only,
    );
    let (minimum_schema, maximum_schema, step_schema) = (schema.clone(), schema.clone(), schema);
    let (minimum_options, maximum_options, step_options) =
        (options.clone(), options.clone(), options);
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Spacer @sizing=ItemSize::Fixed(INDENT) />
                <List spacing=ROW_SPACING>
                    <OptionalNumber
                        schema={minimum_schema}
                        id={id}
                        options={minimum_options}
                        bound=Bound::Minimum
                        read_only={minimum_off}
                    />
                    <OptionalNumber
                        schema={maximum_schema}
                        id={id}
                        options={maximum_options}
                        bound=Bound::Maximum
                        read_only={maximum_off}
                    />
                    <OptionalNumber
                        schema={step_schema}
                        id={id}
                        options={step_options}
                        bound=Bound::Step
                        read_only={step_off}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Body content="Scale" />
                        <Select
                            options={view! {
                                <ForEach keys={scale_keys}>
                                    {|index: usize| view! {
                                        <ChoiceOption label={NUMBER_SCALES[index].1} />
                                    }}
                                </ForEach>
                            }}
                            selected={scale}
                            label="Number scale"
                            disabled={scale_off}
                            @test_id={format!("database-schema.number.{id}.scale")}
                            on_change={rescaled}
                        />
                    </List>
                </List>
            </List>
        </List>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Minimum,
    Maximum,
    Step,
}

impl Bound {
    fn label(self) -> &'static str {
        match self {
            Bound::Minimum => "Minimum",
            Bound::Maximum => "Maximum",
            Bound::Step => "Step",
        }
    }

    fn test_id(self) -> &'static str {
        match self {
            Bound::Minimum => "minimum",
            Bound::Maximum => "maximum",
            Bound::Step => "step",
        }
    }

    fn read(self, options: DatabaseNumberOptions) -> Option<f64> {
        match self {
            Bound::Minimum => options.minimum,
            Bound::Maximum => options.maximum,
            Bound::Step => options.step,
        }
    }

    fn fallback(self, options: DatabaseNumberOptions) -> f64 {
        match self {
            Bound::Minimum => 0.0,
            Bound::Maximum => 100.0,
            Bound::Step => options.effective_step(),
        }
    }

    fn write(self, options: &mut DatabaseNumberOptions, value: Option<f64>) {
        match self {
            Bound::Minimum => options.minimum = value,
            Bound::Maximum => options.maximum = value,
            Bound::Step => options.step = value,
        }
    }
}

#[component]
fn OptionalNumber(
    schema: Schema,
    id: Uuid,
    options: Memo<DatabaseNumberOptions>,
    bound: Bound,
    read_only: Memo<bool>,
) -> NodeId {
    let set = create_memo(clone!(options -> move || bound.read(options.get()).is_some()));
    let value = create_memo(clone!(options -> move || {
        let options = options.get();
        bound.read(options).unwrap_or_else(|| bound.fallback(options))
    }));
    let unset = create_memo(clone!(set read_only -> move || !set.get() || read_only.get()));
    let write = {
        let schema = schema.clone();
        let options = options.clone();
        move |value: Option<f64>| {
            let mut next = options.get_untracked();
            bound.write(&mut next, value);
            schema.operate(DatabaseSchemaOperation::SetNumberOptions {
                field_id: id,
                options: next,
            });
        }
    };
    let toggled = {
        let write = write.clone();
        let options = options.clone();
        move |enabled: bool| {
            let fallback = bound.fallback(options.get_untracked());
            write(enabled.then_some(fallback));
        }
    };
    let typed = move |value: f64| write(Some(value));
    let toggle_off = read_only;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Checkbox
                label={bound.label()}
                checked={set}
                disabled={toggle_off}
                @test_id={format!("database-schema.number.{id}.{}.set", bound.test_id())}
                on_change={toggled}
            />
            <Frame width=NUMBER_WIDTH>
                <NumberInput
                    value={value}
                    label={bound.label()}
                    disabled={unset}
                    @test_id={format!("database-schema.number.{id}.{}", bound.test_id())}
                    on_change={typed}
                />
            </Frame>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn EnumOptions(
    schema: Schema,
    id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
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
    let add_off = read_only.clone();
    let add = clone!(schema -> move || {
        schema.operate(DatabaseSchemaOperation::AddEnumOption {
            field_id: id,
            option: DatabaseEnumOption {
                id: Uuid::new_v4(),
                name: "Option".into(),
            },
        });
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Spacer @sizing=ItemSize::Fixed(INDENT) />
            <List spacing=ROW_SPACING>
                <ForEach keys={keys}>
                    {move |option_id: Uuid| {
                        let name = create_memo(clone!(options option_id -> move || {
                            options.with(|options| {
                                options
                                    .iter()
                                    .find(|option| option.id == option_id)
                                    .map_or_else(String::new, |option| option.name.clone())
                            })
                        }));
                        view! {
                            <EnumOptionRow
                                schema={schema.clone()}
                                id={id}
                                option_id={option_id}
                                name={name}
                                read_only={read_only.clone()}
                            />
                        }
                    }}
                </ForEach>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <IconButton
                        glyph={ICON_ADD.to_owned()}
                        label="Add option"
                        disabled={add_off}
                        @test_id={format!("database-schema.enum.{id}.add")}
                        on_click={add}
                    />
                    <Body content="Add option" />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </List>
    }
}

#[component]
fn EnumOptionRow(
    schema: Schema,
    id: Uuid,
    option_id: Uuid,
    name: Memo<String>,
    read_only: Memo<bool>,
) -> NodeId {
    let renamed = clone!(schema -> move |name: String| {
        schema.operate(DatabaseSchemaOperation::RenameEnumOption {
            field_id: id,
            option_id,
            name,
        });
    });
    let remove = clone!(schema -> move || {
        schema.operate(DatabaseSchemaOperation::RemoveEnumOption {
            field_id: id,
            option_id,
        });
    });
    let (name_off, delete_off) = (read_only.clone(), read_only);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Frame width=NAME_WIDTH>
                <TextInput
                    value={name}
                    label="Option name"
                    disabled={name_off}
                    @test_id={format!("database-schema.enum.{id}.{option_id}.name")}
                    on_change={renamed}
                />
            </Frame>
            <IconButton
                glyph={ICON_DELETE.to_owned()}
                label="Delete option"
                disabled={delete_off}
                @test_id={format!("database-schema.enum.{id}.{option_id}.delete")}
                on_click={remove}
            />
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn BlockOptions(
    editor: Editor,
    schema: Schema,
    id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
) -> NodeId {
    let catalog = editor.block_types();
    let mut known: Vec<(Uuid, String)> = catalog
        .iter()
        .map(|(id, entry)| (*id, entry.display_name.clone()))
        .collect();
    known.sort_by(|(left_id, left), (right_id, right)| {
        left.cmp(right).then_with(|| left_id.cmp(right_id))
    });
    let wanted = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().and_then(|field| field.block_options.block_type))
    }));
    let entries = create_memo(clone!(wanted known -> move || {
        let mut entries: Vec<(Option<Uuid>, String)> = vec![(None, "Any block".to_owned())];
        entries.extend(known.iter().map(|(id, name)| (Some(*id), name.clone())));
        match wanted.get() {
            Some(unknown) if !known.iter().any(|(id, _)| *id == unknown) => {
                entries.push((Some(unknown), format!("Unknown type ({unknown})")));
            }
            _ => {}
        }
        entries
    }));
    let keys = create_memo(clone!(entries -> move || {
        entries.with(|entries| entries.iter().map(|(id, _)| *id).collect::<Vec<Option<Uuid>>>())
    }));
    let selected = create_memo(clone!(entries wanted -> move || {
        let wanted = wanted.get();
        entries.with(|entries| entries.iter().position(|(value, _)| *value == wanted))
    }));
    let labels = entries.clone();
    let picked = clone!(schema entries -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let Some(block_type) = entries.with_untracked(|entries| {
            entries.get(index).map(|(block_type, _)| *block_type)
        }) else {
            return;
        };
        schema.operate(DatabaseSchemaOperation::SetBlockOptions {
            field_id: id,
            options: DatabaseBlockOptions { block_type },
        });
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Spacer @sizing=ItemSize::Fixed(INDENT) />
            <Body content="Block type" />
            <Select
                options={view! {
                    <ForEach keys={keys}>
                        {move |id: Option<Uuid>| {
                            let label = create_memo(clone!(labels id -> move || {
                                labels.with(|entries| {
                                    entries
                                        .iter()
                                        .find(|(value, _)| *value == id)
                                        .map_or_else(String::new, |(_, name)| name.clone())
                                })
                            }));
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }}
                    </ForEach>
                }}
                selected={selected}
                label="Block type"
                disabled={read_only}
                @test_id={format!("database-schema.block.{id}.type")}
                on_change={picked}
            />
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}
