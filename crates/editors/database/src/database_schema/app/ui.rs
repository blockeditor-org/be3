use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::Edit;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::styled::context_menu::menu_style;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, MenuItem, PopupTriggerHandle};
use block_editor_beui::database::{
    ChoiceMark, choice_color_label, choice_glyph, choice_icon_label,
};
use database_types::database::DatabaseContent;
use database_types::database_schema::{
    ChoiceColor, ChoiceIcon, DatabaseBlockOptions, DatabaseEnumOption, DatabaseField,
    DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale, DatabaseRelationOptions,
    DatabaseSchema, DatabaseSchemaContent,
};

use crate::database_view::app::controls::ControlFace;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_BLOCKS, ICON_CALENDAR, ICON_CIRCLE_CHEVRON_DOWN,
    ICON_HASH, ICON_LINK, ICON_PALETTE, ICON_PLUS, ICON_SQUARE_CHECK, ICON_TEXT_ALIGN_START,
    ICON_TRASH,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{
    BlockFilter, ChildTarget, ContentProjection, Editor, node_anchor, watch_block_label,
};
use uuid::Uuid;

use super::field_line_count;
use block_editor_beui::beui::TextAlign;

#[derive(Clone, Copy)]
struct Schema(ContentProjection<DatabaseSchemaContent>);

impl Schema {
    fn operate(self, edit: Edit) {
        self.0.operate(edit);
    }

    fn project<T: Clone + PartialEq + 'static>(
        self,
        project: impl Fn(&DatabaseSchemaContent) -> T + 'static,
    ) -> ReadSignal<T> {
        self.0.project(project)
    }
}

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const INDENT: f32 = 16.0;
const NAME_WIDTH: f32 = 240.0;
const NUMBER_WIDTH: f32 = 120.0;
const INTRINSIC_WIDTH: f32 = 600.0;
const ROW_HEIGHT: f32 = 40.0;

const FIELD_TYPES: [(DatabaseFieldType, &str, Glyph); 8] = [
    (DatabaseFieldType::String, "Text", ICON_TEXT_ALIGN_START),
    (DatabaseFieldType::Number, "Number", ICON_HASH),
    (DatabaseFieldType::Enum, "Choice", ICON_CIRCLE_CHEVRON_DOWN),
    (DatabaseFieldType::Block, "Block", ICON_BLOCKS),
    (DatabaseFieldType::Boolean, "Checkbox", ICON_SQUARE_CHECK),
    (DatabaseFieldType::Color, "Color", ICON_PALETTE),
    (DatabaseFieldType::Datetime, "Date and time", ICON_CALENDAR),
    (DatabaseFieldType::Relation, "Link to rows", ICON_LINK),
];

const NUMBER_SCALES: [(DatabaseNumberScale, &str); 2] = [
    (DatabaseNumberScale::Linear, "Linear"),
    (DatabaseNumberScale::Logarithmic, "Logarithmic"),
];

#[component]
pub fn SchemaView(editor: Editor) -> NodeId {
    let schema = editor.block_content::<DatabaseSchemaContent>();
    let fields = schema.project(|schema| schema.root().fields());
    let schema = Schema(schema);
    let rows = create_memo(move || fields.get());
    let keys = create_memo(move || {
        rows.with(|rows| rows.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    });
    let read_only = editor.read_only();

    create_effect(move || {
        let (lines, count) = rows.with(|rows| {
            (
                rows.iter().map(field_line_count).sum::<usize>() + 2,
                rows.len(),
            )
        });
        let height = ROW_HEIGHT * lines as f32 + SECTION_SPACING * count as f32;
        editor.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    let add = move || {
        schema.operate(DatabaseSchema::add_field("Field", DatabaseFieldType::String).1);
    };

    view! {
        <Frame padding=PADDING>
            <List spacing=SECTION_SPACING>
                <Text style=TextStyle::HEADING>"Fields"</Text>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <ForEach keys={keys}>|id: Uuid| {
                            let field = create_memo(move || {
                                rows.with(|rows| {
                                    rows.iter().find(|field| field.id == id).cloned()
                                })
                            });
                            view! {
                                <FieldRow
                                    editor={editor}
                                    schema={schema}
                                    id={id}
                                    field={field}
                                    read_only={read_only}
                                />
                            }
                        }</ForEach>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=ROW_SPACING
                        >
                            <Button
                                label="Add field"
                                variant=ButtonVariant::Primary
                                disabled={read_only}
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
    let name = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map_or_else(String::new, |field| field.name.clone())
        })
    });
    let kind =
        create_memo(move || field.with(|field| field.as_ref().map(|field| field.field_type)));
    let selected = create_memo(move || {
        kind.get()
            .and_then(|kind| FIELD_TYPES.iter().position(|(value, _, _)| *value == kind))
    });
    let is = |wanted: DatabaseFieldType| create_memo(move || kind.get() == Some(wanted));
    let renamed = move |name: String| {
        schema.operate(DatabaseSchema::rename_field(id, name));
    };
    let retyped = move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        schema.operate(DatabaseSchema::set_field_type(id, FIELD_TYPES[index].0));
    };
    let remove = move || {
        schema.operate(DatabaseSchema::remove_field(id));
    };
    let glyph = create_memo(move || {
        kind.get()
            .and_then(|kind| FIELD_TYPES.iter().find(|(value, _, _)| *value == kind))
            .map(|(_, _, glyph)| *glyph)
    });
    let order = schema.project(|schema| {
        schema
            .root()
            .fields
            .iter()
            .map(|field| field.id.as_uuid())
            .collect::<Vec<Uuid>>()
    });
    let mover = move |by: isize| {
        let order = order.get_untracked();
        let Some(at) = order.iter().position(|held| *held == id) else {
            return;
        };
        let Some(to) = at.checked_add_signed(by).filter(|to| *to < order.len()) else {
            return;
        };
        let mut moved = order.clone();
        moved.remove(at);
        let after = to.checked_sub(1).map(|before| moved[before]);
        schema.operate(DatabaseSchema::move_field(id, after));
    };
    let type_keys = create_memo(|| (0..FIELD_TYPES.len()).collect::<Vec<usize>>());
    let (number_kind, enum_kind, block_kind, relation_kind) = (
        is(DatabaseFieldType::Number),
        is(DatabaseFieldType::Enum),
        is(DatabaseFieldType::Block),
        is(DatabaseFieldType::Relation),
    );
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text size=TextSize::Lg color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Frame width=NAME_WIDTH>
                    <TextInput
                        value={Binding::new(name, renamed)}
                        label="Field name"
                        disabled={read_only}
                        @test_id={format!("database-schema.name.{id}")}
                    />
                </Frame>
                <Select
                    options={view! {
                        <ForEach keys={type_keys}>|index: usize|
                            <ChoiceOption label={FIELD_TYPES[index].1} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected, retyped)}
                    label="Field type"
                    disabled={read_only}
                    @test_id={format!("database-schema.type.{id}")}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_UP
                    label="Move up"
                    variant=ButtonVariant::Ghost
                    disabled={read_only}
                    @test_id={format!("database-schema.up.{id}")}
                    on_click={move || mover(-1)}
                />
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_DOWN
                    label="Move down"
                    variant=ButtonVariant::Ghost
                    disabled={read_only}
                    @test_id={format!("database-schema.down.{id}")}
                    on_click={move || mover(1)}
                />
                <Button
                    icon_only=true
                    glyph=ICON_TRASH
                    label="Delete field"
                    disabled={read_only}
                    @test_id={format!("database-schema.delete.{id}")}
                    on_click={remove}
                />
            </List>
            <Show condition={number_kind}>
                <NumberOptions schema={schema} id={id} field={field} read_only={read_only} />
            </Show>
            <Show condition={enum_kind}>
                <EnumOptions schema={schema} id={id} field={field} read_only={read_only} />
            </Show>
            <Show condition={block_kind}>
                <BlockOptions
                    editor={editor}
                    schema={schema}
                    id={id}
                    field={field}
                    read_only={read_only}
                />
            </Show>
            <Show condition={relation_kind}>
                <RelationOptions
                    editor={editor}
                    schema={schema}
                    id={id}
                    field={field}
                    read_only={read_only}
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
    let options = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.number_options)
                .unwrap_or_default()
        })
    });
    let scale = create_memo(move || {
        NUMBER_SCALES
            .iter()
            .position(|(value, _)| *value == options.get().scale)
    });
    let rescaled = move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let mut next = options.get_untracked();
        next.scale = NUMBER_SCALES[index].0;
        schema.operate(DatabaseSchema::set_number_options(id, next));
    };
    let scale_keys = create_memo(|| (0..NUMBER_SCALES.len()).collect::<Vec<usize>>());
    view! {
        <List spacing=ROW_SPACING>
            <Frame padding={Sides::new(INDENT + ROW_SPACING, 0.0, 0.0, 0.0)}>
                <List spacing=ROW_SPACING>
                    <OptionalNumber
                        schema={schema}
                        id={id}
                        options={options}
                        bound=Bound::Minimum
                        read_only={read_only}
                    />
                    <OptionalNumber
                        schema={schema}
                        id={id}
                        options={options}
                        bound=Bound::Maximum
                        read_only={read_only}
                    />
                    <OptionalNumber
                        schema={schema}
                        id={id}
                        options={options}
                        bound=Bound::Step
                        read_only={read_only}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Text>"Scale"</Text>
                        <Select
                            options={view! {
                                <ForEach keys={scale_keys}>|index: usize|
                                    <ChoiceOption label={NUMBER_SCALES[index].1} />
                                </ForEach>
                            }}
                            selected={Binding::new(scale, rescaled)}
                            label="Number scale"
                            disabled={read_only}
                            @test_id={format!("database-schema.number.{id}.scale")}
                        />
                    </List>
                </List>
            </Frame>
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
    let set = create_memo(move || bound.read(options.get()).is_some());
    let value = create_memo(move || {
        let options = options.get();
        bound
            .read(options)
            .unwrap_or_else(|| bound.fallback(options))
    });
    let unset = create_memo(move || !set.get() || read_only.get());
    let write = move |value: Option<f64>| {
        let mut next = options.get_untracked();
        bound.write(&mut next, value);
        schema.operate(DatabaseSchema::set_number_options(id, next));
    };
    let toggled = move |enabled: bool| {
        let fallback = bound.fallback(options.get_untracked());
        write(enabled.then_some(fallback));
    };
    let typed = move |value: f64| write(Some(value));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Checkbox
                label={bound.label()}
                checked={Binding::new(set, toggled)}
                disabled={read_only}
                @test_id={format!("database-schema.number.{id}.{}.set", bound.test_id())}
            />
            <Frame width=NUMBER_WIDTH>
                <NumberInput
                    value={Binding::new(value, typed)}
                    label={bound.label()}
                    disabled={unset}
                    @test_id={format!("database-schema.number.{id}.{}", bound.test_id())}
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
    let keys = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| {
                    field
                        .enum_options
                        .iter()
                        .map(|option| option.id)
                        .collect::<Vec<Uuid>>()
                })
                .unwrap_or_default()
        })
    });
    let add = move || {
        let count = field
            .with_untracked(|field| field.as_ref().map_or(0, |field| field.enum_options.len()));
        schema.operate(DatabaseSchema::add_enum_option(id, format!("Option {}", count + 1)).1);
    };
    view! {
        <Frame padding={Sides::new(INDENT + ROW_SPACING, 0.0, 0.0, 0.0)}>
            <List spacing=ROW_SPACING>
                <ForEach keys={keys}>|option_id: Uuid|
                    <EnumOptionRow
                        schema={schema}
                        id={id}
                        option_id={option_id}
                        field={field}
                        read_only={read_only}
                    />
                </ForEach>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Button
                        icon_only=true
                        glyph=ICON_PLUS
                        label="Add option"
                        disabled={read_only}
                        @test_id={format!("database-schema.enum.{id}.add")}
                        on_click={add}
                    />
                    <Text>"Add option"</Text>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <Text style=TextStyle::CAPTION>
                    "A closed option marks a row finished: views can hide closed rows, the board folds their columns, and a merged pull request moves rows to the first one. New rows start at the default."
                </Text>
            </List>
        </Frame>
    }
}

#[component]
fn EnumOptionRow(
    schema: Schema,
    id: Uuid,
    option_id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
) -> NodeId {
    let option =
        create_memo(move || field.with(|field| field.as_ref()?.option(option_id).cloned()));
    let name = create_memo(move || {
        option.with(|option| {
            option
                .as_ref()
                .map(|option| option.name.clone())
                .unwrap_or_default()
        })
    });
    let closed = create_memo(move || {
        option.with(|option| option.as_ref().is_some_and(|option| option.closed))
    });
    let default = create_memo(move || {
        field
            .with(|field| field.as_ref().and_then(|field| field.default_value()) == Some(option_id))
    });
    let renamed = move |name: String| {
        schema.operate(DatabaseSchema::rename_enum_option(option_id, name));
    };
    let remove = move || {
        schema.operate(DatabaseSchema::remove_enum_option(option_id));
    };
    let close = move |on: bool| {
        schema.operate(DatabaseSchema::set_option_closed(option_id, on));
    };
    let defaulted = move |on: bool| {
        schema.operate(DatabaseSchema::set_default_option(
            id,
            on.then_some(option_id),
        ));
    };
    let mover = move |by: isize| {
        let order: Vec<Uuid> = field.with_untracked(|field| {
            field
                .as_ref()
                .map(|field| {
                    field
                        .enum_options
                        .iter()
                        .map(|option| option.id)
                        .collect::<Vec<Uuid>>()
                })
                .unwrap_or_default()
        });
        let Some(at) = order.iter().position(|held| *held == option_id) else {
            return;
        };
        let Some(to) = at.checked_add_signed(by).filter(|to| *to < order.len()) else {
            return;
        };
        let mut moved = order.clone();
        moved.remove(at);
        let after = to.checked_sub(1).map(|before| moved[before]);
        schema.operate(DatabaseSchema::move_enum_option(id, option_id, after));
    };
    let icon_keys = create_memo(|| (0..ChoiceIcon::ALL.len()).collect::<Vec<usize>>());
    let color_keys = create_memo(|| (0..ChoiceColor::ALL.len()).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
            <unstyled::MenuButton
                label="Icon"
                disabled={read_only}
                menu={menu_style()}
                @test_id={format!("database-schema.enum.{id}.{option_id}.icon")}
                items={view! {
                    <ForEach keys={icon_keys}>|index: usize| {
                        let icon = ChoiceIcon::ALL[index];
                        view! {
                            <MenuItem
                                label={choice_icon_label(icon)}
                                glyph={choice_glyph(icon)}
                                row_test_id={format!("database-schema.enum.{id}.{option_id}.icon.{}", icon.name())}
                                on_click={move || schema.operate(DatabaseSchema::set_option_icon(option_id, icon))}
                            />
                        }
                    }</ForEach>
                }}
                trigger={move |handle: PopupTriggerHandle| {
                    view! {
                        <ControlFace handle={handle.button}>
                            <ChoiceMark option={option} size=TextSize::Base />
                        </ControlFace>
                    }
                }}
            />
            <Frame width=NAME_WIDTH>
                <TextInput
                    value={Binding::new(name, renamed)}
                    label="Option name"
                    disabled={read_only}
                    select_on_focus=true
                    @test_id={format!("database-schema.enum.{id}.{option_id}.name")}
                />
            </Frame>
            <unstyled::MenuButton
                label="Colour"
                disabled={read_only}
                menu={menu_style()}
                @test_id={format!("database-schema.enum.{id}.{option_id}.color")}
                items={view! {
                    <ForEach keys={color_keys}>|index: usize| {
                        let color = ChoiceColor::ALL[index];
                        view! {
                            <MenuItem
                                label={choice_color_label(color)}
                                row_test_id={format!("database-schema.enum.{id}.{option_id}.color.{}", color.name())}
                                on_click={move || schema.operate(DatabaseSchema::set_option_color(option_id, color))}
                            />
                        }
                    }</ForEach>
                }}
                trigger={move |handle: PopupTriggerHandle| {
                    let label = create_memo(move || {
                        option.with(|option| option.as_ref().map_or("", |option| choice_color_label(option.color)).to_owned())
                    });
                    let dotted = create_memo(move || {
                        option.with(|option| option.as_ref().map(|option| DatabaseEnumOption { icon: ChoiceIcon::None, ..option.clone() }))
                    });
                    view! {
                        <ControlFace handle={handle.button}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                                <ChoiceMark option={dotted} size=TextSize::Sm />
                                <Text>{label}</Text>
                            </List>
                        </ControlFace>
                    }
                }}
            />
            <ToggleButton
                label="Closed"
                pressed={Binding::new(closed, close)}
                disabled={read_only}
                @test_id={format!("database-schema.enum.{id}.{option_id}.closed")}
            />
            <ToggleButton
                label="Default"
                pressed={Binding::new(default, defaulted)}
                disabled={read_only}
                @test_id={format!("database-schema.enum.{id}.{option_id}.default")}
            />
            <Button
                icon_only=true
                glyph=ICON_ARROW_UP
                label="Move up"
                variant=ButtonVariant::Ghost
                disabled={read_only}
                @test_id={format!("database-schema.enum.{id}.{option_id}.up")}
                on_click={move || mover(-1)}
            />
            <Button
                icon_only=true
                glyph=ICON_ARROW_DOWN
                label="Move down"
                variant=ButtonVariant::Ghost
                disabled={read_only}
                @test_id={format!("database-schema.enum.{id}.{option_id}.down")}
                on_click={move || mover(1)}
            />
            <Button
                icon_only=true
                glyph=ICON_TRASH
                label="Delete option"
                variant=ButtonVariant::Ghost
                disabled={read_only}
                @test_id={format!("database-schema.enum.{id}.{option_id}.delete")}
                on_click={remove}
            />
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
    let known = create_memo(move || {
        let mut known: Vec<(Uuid, String)> = editor
            .block_types()
            .iter()
            .map(|(id, entry)| (*id, entry.display_name.clone()))
            .collect();
        known.sort_by(|(left_id, left), (right_id, right)| {
            left.cmp(right).then_with(|| left_id.cmp(right_id))
        });
        known
    });
    let wanted = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .and_then(|field| field.block_options.block_type)
        })
    });
    let entries = create_memo(move || {
        let mut entries: Vec<(Option<Uuid>, String)> = vec![(None, "Any block".to_owned())];
        let known = known.get();
        entries.extend(known.iter().map(|(id, name)| (Some(*id), name.clone())));
        match wanted.get() {
            Some(unknown) if !known.iter().any(|(id, _)| *id == unknown) => {
                entries.push((Some(unknown), format!("Unknown type ({unknown})")));
            }
            _ => {}
        }
        entries
    });
    let keys = create_memo(move || {
        entries.with(|entries| {
            entries
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<Option<Uuid>>>()
        })
    });
    let selected = create_memo(move || {
        let wanted = wanted.get();
        entries.with(|entries| entries.iter().position(|(value, _)| *value == wanted))
    });
    let picked = move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let Some(block_type) =
            entries.with_untracked(|entries| entries.get(index).map(|(block_type, _)| *block_type))
        else {
            return;
        };
        schema.operate(DatabaseSchema::set_block_options(
            id,
            DatabaseBlockOptions { block_type },
        ));
    };
    view! {
        <Frame padding={Sides::new(INDENT + ROW_SPACING, 0.0, 0.0, 0.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text>"Block type"</Text>
                <Select
                    options={view! {
                        <ForEach keys={keys}>|id: Option<Uuid>| {
                            let label = create_memo(move || {
                                entries.with(|entries| {
                                    entries
                                        .iter()
                                        .find(|(value, _)| *value == id)
                                        .map_or_else(String::new, |(_, name)| name.clone())
                                })
                            });
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }</ForEach>
                    }}
                    selected={Binding::new(selected, picked)}
                    label="Block type"
                    disabled={read_only}
                    @test_id={format!("database-schema.block.{id}.type")}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
fn RelationOptions(
    editor: Editor,
    schema: Schema,
    id: Uuid,
    field: Memo<Option<DatabaseField>>,
    read_only: Memo<bool>,
) -> NodeId {
    let target = create_memo(move || {
        field
            .with(|field| {
                field
                    .as_ref()
                    .and_then(|field| field.relation_options.database)
            })
            .map(|database| ChildTarget::new(database, DatabaseContent::CONTENT_TYPE))
    });
    let shown = watch_block_label(editor, target);
    let label = create_memo(move || match target.get() {
        None => "Choose a database".to_owned(),
        Some(_) => shown.with(|shown| match shown.resolved {
            false => "Loading…".to_owned(),
            true => shown.name.clone(),
        }),
    });
    let options = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.relation_options.clone())
                .unwrap_or_default()
        })
    });
    let button = NodeRef::new();
    let choose = move || {
        let current = options.get_untracked();
        editor.pick_block(
            BlockFilter {
                name: "Linked database".to_owned(),
                block_types: vec![DatabaseContent::CONTENT_TYPE.into_bytes()],
                excluded: Vec::new(),
                templates: false,
                place: None,
                anchor: node_anchor(&button),
            },
            move |picked| {
                if let Ok(picked) = picked {
                    schema.operate(DatabaseSchema::set_relation_options(
                        id,
                        DatabaseRelationOptions {
                            database: Some(picked.id()),
                            ..current.clone()
                        },
                    ));
                }
            },
        );
    };
    let multiple = create_memo(move || options.get().multiple);
    let inverse = create_memo(move || options.get().inverse);
    let remultiplied = move |multiple: bool| {
        schema.operate(DatabaseSchema::set_relation_options(
            id,
            DatabaseRelationOptions {
                multiple,
                ..options.get_untracked()
            },
        ));
    };
    let renamed = move |inverse: String| {
        schema.operate(DatabaseSchema::set_relation_options(
            id,
            DatabaseRelationOptions {
                inverse,
                ..options.get_untracked()
            },
        ));
    };
    view! {
        <Frame padding={Sides::new(INDENT + ROW_SPACING, 0.0, 0.0, 0.0)}>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Text>"Links to"</Text>
                    <Button
                        @node_ref={&button}
                        label={label}
                        variant=ButtonVariant::Secondary
                        disabled={read_only}
                        @test_id={format!("database-schema.relation.{id}.database")}
                        on_click={choose}
                    />
                    <Checkbox
                        label="Several rows"
                        checked={Binding::new(multiple, remultiplied)}
                        disabled={read_only}
                        @test_id={format!("database-schema.relation.{id}.multiple")}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Text>"Shown on the linked rows as"</Text>
                    <Frame width=NAME_WIDTH>
                        <TextInput
                            value={Binding::new(inverse, renamed)}
                            placeholder="No column"
                            disabled={read_only}
                            @test_id={format!("database-schema.relation.{id}.inverse")}
                        />
                    </Frame>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Frame>
    }
}
