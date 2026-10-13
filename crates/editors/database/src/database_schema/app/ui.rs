use std::rc::Rc;

use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::database::DatabaseContent;
use block_editor_beui::be_block::database_schema::{
    ChoiceColor, ChoiceIcon, DatabaseBlockOptions, DatabaseEnumOption, DatabaseField,
    DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale, DatabaseRelationOptions,
    DatabaseSchema, DatabaseSchemaContent,
};
use block_editor_beui::beui::styled::context_menu::menu_style;
use block_editor_beui::beui::styled::{Caption, ToggleButton};
use block_editor_beui::beui::unstyled::{self, MenuButtonHandle, MenuItem};
use block_editor_beui::database::{
    ChoiceMark, choice_color_label, choice_glyph, choice_icon_label,
};

use crate::database_view::app::controls::ControlFace;
use block_editor_beui::beui::icons::{
    ICON_ADD, ICON_ARROW_DOWNWARD, ICON_ARROW_DROP_DOWN_CIRCLE, ICON_ARROW_UPWARD,
    ICON_CALENDAR_TODAY, ICON_CHECK_BOX, ICON_DELETE, ICON_LINK, ICON_NUMBERS, ICON_PALETTE,
    ICON_SHORT_TEXT, ICON_WIDGETS,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_effect, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Checkbox, Heading, IconButton, IconSized, NumberInput, Scroll,
    Select, Separator, TextInput, use_theme,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockFilter, ChildTarget, ContentProjection, Editor, watch_block_label};
use uuid::Uuid;

use super::field_line_count;

type Schema = Rc<ContentProjection<DatabaseSchemaContent>>;

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const INDENT: f32 = 16.0;
const NAME_WIDTH: f32 = 240.0;
const NUMBER_WIDTH: f32 = 120.0;
const INTRINSIC_WIDTH: f32 = 600.0;
const ROW_HEIGHT: f32 = 40.0;

const FIELD_TYPES: [(DatabaseFieldType, &str, &str); 8] = [
    (DatabaseFieldType::String, "Text", ICON_SHORT_TEXT),
    (DatabaseFieldType::Number, "Number", ICON_NUMBERS),
    (
        DatabaseFieldType::Enum,
        "Choice",
        ICON_ARROW_DROP_DOWN_CIRCLE,
    ),
    (DatabaseFieldType::Block, "Block", ICON_WIDGETS),
    (DatabaseFieldType::Boolean, "Checkbox", ICON_CHECK_BOX),
    (DatabaseFieldType::Color, "Color", ICON_PALETTE),
    (
        DatabaseFieldType::Datetime,
        "Date and time",
        ICON_CALENDAR_TODAY,
    ),
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
    let rows = create_memo(move || fields.get());
    let keys = create_memo(move || {
        rows.with(|rows| rows.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    });
    let read_only = editor.read_only();

    let sized = editor.clone();
    create_effect(move || {
        let (lines, count) = rows.with(|rows| {
            (
                rows.iter().map(field_line_count).sum::<usize>() + 2,
                rows.len(),
            )
        });
        let height = ROW_HEIGHT * lines as f32 + SECTION_SPACING * count as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    let add = clone!(schema -> move || {
        schema.operate(DatabaseSchema::add_field("Field", DatabaseFieldType::String).1);
    });

    let add_disabled = read_only;
    let theme = use_theme();
    view! {
        <Frame color={theme.background} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SECTION_SPACING>
                <Heading content="Fields" />
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <ForEach keys={keys}>
                            {move |id: Uuid| {
                                let field = create_memo(move || {
                                    rows.with(|rows| {
                                        rows.iter().find(|field| field.id == id).cloned()
                                    })
                                });
                                view! {
                                    <FieldRow
                                        editor={editor.clone()}
                                        schema={schema.clone()}
                                        id={id}
                                        field={field}
                                        read_only={read_only}
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
    let renamed = clone!(schema -> move |name: String| {
        schema.operate(DatabaseSchema::rename_field(id, name));
    });
    let retyped = clone!(schema -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        schema.operate(DatabaseSchema::set_field_type(id, FIELD_TYPES[index].0));
    });
    let remove = clone!(schema -> move || {
        schema.operate(DatabaseSchema::remove_field(id));
    });
    let glyph = create_memo(move || {
        kind.get()
            .and_then(|kind| FIELD_TYPES.iter().find(|(value, _, _)| *value == kind))
            .map_or("", |(_, _, glyph)| glyph)
            .to_owned()
    });
    let order = schema.project(|schema| {
        schema
            .root()
            .fields
            .iter()
            .map(|field| field.id.as_uuid())
            .collect::<Vec<Uuid>>()
    });
    let mover = clone!(schema -> move |by: isize| {
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
    });
    let (up, down) = (mover.clone(), mover);
    let (up_off, down_off) = (read_only, read_only);
    let theme = use_theme();
    let type_keys = create_memo(|| (0..FIELD_TYPES.len()).collect::<Vec<usize>>());
    let (name_off, type_off, delete_off) = (read_only, read_only, read_only);
    let (number_off, enum_off, block_off, relation_off) =
        (read_only, read_only, read_only, read_only);
    let (number_schema, enum_schema, block_schema, relation_schema) =
        (schema.clone(), schema.clone(), schema.clone(), schema);
    let (number_field, enum_field, block_field, relation_field) = (field, field, field, field);
    let relation_editor = editor.clone();
    let (number_kind, enum_kind, block_kind, relation_kind) = (
        is(DatabaseFieldType::Number),
        is(DatabaseFieldType::Enum),
        is(DatabaseFieldType::Block),
        is(DatabaseFieldType::Relation),
    );
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <IconSized glyph={glyph} font_size=20.0 color={theme.text_muted} />
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
                    glyph={ICON_ARROW_UPWARD.to_owned()}
                    label="Move up"
                    variant=ButtonVariant::Ghost
                    disabled={up_off}
                    @test_id={format!("database-schema.up.{id}")}
                    on_click={move || up(-1)}
                />
                <IconButton
                    glyph={ICON_ARROW_DOWNWARD.to_owned()}
                    label="Move down"
                    variant=ButtonVariant::Ghost
                    disabled={down_off}
                    @test_id={format!("database-schema.down.{id}")}
                    on_click={move || down(1)}
                />
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
                    schema={number_schema.clone()}
                    id={id}
                    field={number_field}
                    read_only={number_off}
                />
            </Show>
            <Show condition={enum_kind}>
                <EnumOptions
                    schema={enum_schema.clone()}
                    id={id}
                    field={enum_field}
                    read_only={enum_off}
                />
            </Show>
            <Show condition={block_kind}>
                <BlockOptions
                    editor={editor.clone()}
                    schema={block_schema.clone()}
                    id={id}
                    field={block_field}
                    read_only={block_off}
                />
            </Show>
            <Show condition={relation_kind}>
                <RelationOptions
                    editor={relation_editor.clone()}
                    schema={relation_schema.clone()}
                    id={id}
                    field={relation_field}
                    read_only={relation_off}
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
    let rescaled = clone!(schema -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let mut next = options.get_untracked();
        next.scale = NUMBER_SCALES[index].0;
        schema.operate(DatabaseSchema::set_number_options(id, next));
    });
    let scale_keys = create_memo(|| (0..NUMBER_SCALES.len()).collect::<Vec<usize>>());
    let (minimum_off, maximum_off, step_off, scale_off) =
        (read_only, read_only, read_only, read_only);
    let (minimum_schema, maximum_schema, step_schema) = (schema.clone(), schema.clone(), schema);
    let (minimum_options, maximum_options, step_options) = (options, options, options);
    view! {
        <List spacing=ROW_SPACING>
            <Frame padding_left={INDENT + ROW_SPACING}>
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
    let write = {
        let schema = schema.clone();
        let options = options;
        move |value: Option<f64>| {
            let mut next = options.get_untracked();
            bound.write(&mut next, value);
            schema.operate(DatabaseSchema::set_number_options(id, next));
        }
    };
    let toggled = {
        let write = write.clone();
        let options = options;
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
    let keys = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map(|field| field.enum_options.iter().map(|option| option.id).collect())
                .unwrap_or_default()
        })
    });
    let add_off = read_only;
    let add = clone!(schema -> move || {
        let count = field.with_untracked(|field| field.as_ref().map_or(0, |field| field.enum_options.len()));
        schema.operate(DatabaseSchema::add_enum_option(id, format!("Option {}", count + 1)).1);
    });
    view! {
        <Frame padding_left={INDENT + ROW_SPACING}>
            <List spacing=ROW_SPACING>
                <ForEach keys={keys}>
                    {move |option_id: Uuid| {
                        view! {
                            <EnumOptionRow
                                schema={schema.clone()}
                                id={id}
                                option_id={option_id}
                                field={field}
                                read_only={read_only}
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
                <Caption
                    content="A closed option marks a row finished: views can hide closed rows, the board folds their columns, and a merged pull request moves rows to the first one. New rows start at the default."
                    wrap=true
                />
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
    let renamed = clone!(schema -> move |name: String| {
        schema.operate(DatabaseSchema::rename_enum_option(option_id, name));
    });
    let remove = clone!(schema -> move || {
        schema.operate(DatabaseSchema::remove_enum_option(option_id));
    });
    let close = clone!(schema -> move |on: bool| {
        schema.operate(DatabaseSchema::set_option_closed(option_id, on));
    });
    let defaulted = clone!(schema -> move |on: bool| {
        schema.operate(DatabaseSchema::set_default_option(id, on.then_some(option_id)));
    });
    let mover = clone!(schema -> move |by: isize| {
        let order: Vec<Uuid> = field.with_untracked(|field| {
            field.as_ref().map(|field| field.enum_options.iter().map(|option| option.id).collect()).unwrap_or_default()
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
    });
    let (up, down) = (mover.clone(), mover);
    let icon_keys = create_memo(|| (0..ChoiceIcon::ALL.len()).collect::<Vec<usize>>());
    let color_keys = create_memo(|| (0..ChoiceColor::ALL.len()).collect::<Vec<usize>>());
    let (icon_schema, color_schema) = (schema.clone(), schema);
    let (name_off, delete_off, icon_off, color_off, closed_off, default_off, up_off, down_off) = (
        read_only, read_only, read_only, read_only, read_only, read_only, read_only, read_only,
    );
    let color_option = option;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
            <unstyled::MenuButton
                label="Icon"
                disabled={icon_off}
                menu={menu_style()}
                @test_id={format!("database-schema.enum.{id}.{option_id}.icon")}
                items={view! {
                    <ForEach keys={icon_keys}>
                        {move |index: usize| {
                            let icon = ChoiceIcon::ALL[index];
                            let schema = icon_schema.clone();
                            view! {
                                <MenuItem
                                    label={choice_icon_label(icon)}
                                    glyph={choice_glyph(icon).to_owned()}
                                    row_test_id={format!("database-schema.enum.{id}.{option_id}.icon.{}", icon.name())}
                                    on_click={move || schema.operate(DatabaseSchema::set_option_icon(option_id, icon))}
                                />
                            }
                        }}
                    </ForEach>
                }}
                trigger={move |handle: MenuButtonHandle| {
                    let option = option;
                    view! {
                        <ControlFace handle={handle.button}>
                            <ChoiceMark option={option} size=18.0 />
                        </ControlFace>
                    }
                }}
            />
            <Frame width=NAME_WIDTH>
                <TextInput
                    value={name}
                    label="Option name"
                    disabled={name_off}
                    select_on_focus=true
                    @test_id={format!("database-schema.enum.{id}.{option_id}.name")}
                    on_change={renamed}
                />
            </Frame>
            <unstyled::MenuButton
                label="Colour"
                disabled={color_off}
                menu={menu_style()}
                @test_id={format!("database-schema.enum.{id}.{option_id}.color")}
                items={view! {
                    <ForEach keys={color_keys}>
                        {move |index: usize| {
                            let color = ChoiceColor::ALL[index];
                            let schema = color_schema.clone();
                            view! {
                                <MenuItem
                                    label={choice_color_label(color)}
                                    row_test_id={format!("database-schema.enum.{id}.{option_id}.color.{}", color.name())}
                                    on_click={move || schema.operate(DatabaseSchema::set_option_color(option_id, color))}
                                />
                            }
                        }}
                    </ForEach>
                }}
                trigger={move |handle: MenuButtonHandle| {
                    let label = create_memo(move || {
                        color_option.with(|option| option.as_ref().map_or("", |option| choice_color_label(option.color)).to_owned())
                    });
                    let dotted = create_memo(move || {
                        color_option.with(|option| option.as_ref().map(|option| DatabaseEnumOption { icon: ChoiceIcon::None, ..option.clone() }))
                    });
                    view! {
                        <ControlFace handle={handle.button}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                                <ChoiceMark option={dotted} size=16.0 />
                                <Body content={label} />
                            </List>
                        </ControlFace>
                    }
                }}
            />
            <ToggleButton
                label="Closed"
                pressed={closed}
                disabled={closed_off}
                @test_id={format!("database-schema.enum.{id}.{option_id}.closed")}
                on_change={close}
            />
            <ToggleButton
                label="Default"
                pressed={default}
                disabled={default_off}
                @test_id={format!("database-schema.enum.{id}.{option_id}.default")}
                on_change={defaulted}
            />
            <IconButton
                glyph={ICON_ARROW_UPWARD.to_owned()}
                label="Move up"
                variant=ButtonVariant::Ghost
                disabled={up_off}
                @test_id={format!("database-schema.enum.{id}.{option_id}.up")}
                on_click={move || up(-1)}
            />
            <IconButton
                glyph={ICON_ARROW_DOWNWARD.to_owned()}
                label="Move down"
                variant=ButtonVariant::Ghost
                disabled={down_off}
                @test_id={format!("database-schema.enum.{id}.{option_id}.down")}
                on_click={move || down(1)}
            />
            <IconButton
                glyph={ICON_DELETE.to_owned()}
                label="Delete option"
                variant=ButtonVariant::Ghost
                disabled={delete_off}
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
    let catalog = editor.block_types();
    let mut known: Vec<(Uuid, String)> = catalog
        .iter()
        .map(|(id, entry)| (*id, entry.display_name.clone()))
        .collect();
    known.sort_by(|(left_id, left), (right_id, right)| {
        left.cmp(right).then_with(|| left_id.cmp(right_id))
    });
    let wanted = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .and_then(|field| field.block_options.block_type)
        })
    });
    let entries = create_memo(clone!(known -> move || {
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
    let labels = entries;
    let picked = clone!(schema -> move |selected: Option<usize>| {
        let Some(index) = selected else {
            return;
        };
        let Some(block_type) = entries.with_untracked(|entries| {
            entries.get(index).map(|(block_type, _)| *block_type)
        }) else {
            return;
        };
        schema.operate(DatabaseSchema::set_block_options(
            id,
            DatabaseBlockOptions { block_type },
        ));
    });
    view! {
        <Frame padding_left={INDENT + ROW_SPACING}>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Body content="Block type" />
                <Select
                    options={view! {
                        <ForEach keys={keys}>
                            {move |id: Option<Uuid>| {
                                let label = create_memo(move || {
                                    labels.with(|entries| {
                                        entries
                                            .iter()
                                            .find(|(value, _)| *value == id)
                                            .map_or_else(String::new, |(_, name)| name.clone())
                                    })
                                });
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
    let shown = watch_block_label(&editor, target);
    let label = create_memo(move || match target.get() {
        None => "Choose a database".to_owned(),
        Some(_) => shown.with(|shown| match shown.name.is_empty() {
            true => "Loading…".to_owned(),
            false => shown.name.clone(),
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
    let choose = clone!(editor schema -> move || {
        let schema = schema.clone();
        let current = options.get_untracked();
        editor.pick_block(
            BlockFilter {
                name: "Linked database".to_owned(),
                block_types: vec![DatabaseContent::CONTENT_TYPE.into_bytes()],
                excluded: Vec::new(),
                templates: false,
                place: None,
            },
            move |picked| {
                if let Ok(picked) = picked {
                    schema.operate(DatabaseSchema::set_relation_options(
                        id,
                        DatabaseRelationOptions {
                            database: Some(picked.id),
                            ..current.clone()
                        },
                    ));
                }
            },
        );
    });
    let multiple = create_memo(move || options.get().multiple);
    let inverse = create_memo(move || options.get().inverse);
    let remultiplied = clone!(schema -> move |multiple: bool| {
        schema.operate(DatabaseSchema::set_relation_options(
            id,
            DatabaseRelationOptions {
                multiple,
                ..options.get_untracked()
            },
        ));
    });
    let renamed = clone!(schema -> move |inverse: String| {
        schema.operate(DatabaseSchema::set_relation_options(
            id,
            DatabaseRelationOptions {
                inverse,
                ..options.get_untracked()
            },
        ));
    });
    let (multiple_off, inverse_off) = (read_only, read_only);
    view! {
        <Frame padding_left={INDENT + ROW_SPACING}>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Body content="Links to" />
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        disabled={read_only}
                        @test_id={format!("database-schema.relation.{id}.database")}
                        on_click={choose}
                    />
                    <Checkbox
                        label="Several rows"
                        checked={multiple}
                        disabled={multiple_off}
                        @test_id={format!("database-schema.relation.{id}.multiple")}
                        on_change={remultiplied}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Body content="Shown on the linked rows as" />
                    <Frame width=NAME_WIDTH>
                        <TextInput
                            value={inverse}
                            placeholder="No column"
                            disabled={inverse_off}
                            @test_id={format!("database-schema.relation.{id}.inverse")}
                            on_change={renamed}
                        />
                    </Frame>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Frame>
    }
}
