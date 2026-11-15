use be_sdk_beui::BlockQuery;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CALENDAR, ICON_CHART_SCATTER, ICON_GRID_3X3, ICON_KANBAN,
    ICON_LIST, ICON_TABLE_PROPERTIES,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_database::database_schema::{DatabaseField, DatabaseFieldType};
use be_type_database::database_view::{
    DatabaseView, DatabaseViewKind, DatabaseViewSort, SortDirection,
};
use uuid::Uuid;

use crate::database_view::groups::groupable;
use crate::database_view::root::data::Data;
use beui::{Align, Direction};

const SPACING: Space = Space::Md;
const LABEL_WIDTH: f32 = 96.0;
pub const KINDS: [(DatabaseViewKind, &str, Glyph); 5] = [
    (DatabaseViewKind::List, "List", ICON_LIST),
    (DatabaseViewKind::Kanban, "Board", ICON_KANBAN),
    (DatabaseViewKind::Spreadsheet, "Table", ICON_GRID_3X3),
    (DatabaseViewKind::Scatter, "Scatter", ICON_CHART_SCATTER),
    (DatabaseViewKind::Calendar, "Calendar", ICON_CALENDAR),
];

#[component]
pub fn DisplayOptions(data: Data) -> NodeId {
    let grouped = create_memo(move || {
        matches!(
            data.kind.get(),
            DatabaseViewKind::List | DatabaseViewKind::Kanban
        )
    });
    let ordered = create_memo(move || {
        !matches!(
            data.kind.get(),
            DatabaseViewKind::Scatter | DatabaseViewKind::Calendar
        )
    });
    let prefix = data.prefix;
    let prefix_off = data.read_only;
    let reprefix = move |text: String| data.set_prefix(text);
    let fields_open = move || data.open_schema();
    let shared = create_memo(move || !data.sharing_prefix.with(Vec::is_empty));
    let sharing = create_memo(move || {
        format!(
            "{} is also the prefix of these databases in this workspace, so a row id with it names more than one:",
            prefix.get().trim().to_ascii_uppercase()
        )
    });
    let others = data.sharing_prefix;
    view! {
        <List spacing=SPACING>
            <KindSwitch data={data} />
            <Separator />
            <Show value={grouped}>
                <GroupPicker data={data} />
            </Show>
            <Show value={ordered}>
                <OrderPicker data={data} />
            </Show>
            <Match value={data.kind}>
                <Case is={DatabaseViewKind::Scatter}>
                    <AxisPickers data={data} />
                </Case>
                <Case is={DatabaseViewKind::Calendar}>
                    <DatePickers data={data} />
                </Case>
                <Case is={DatabaseViewKind::List}>
                    <EmptyGroups data={data} />
                    <Separator />
                    <ShownFields data={data} />
                </Case>
            </Match>
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem length=LABEL_WIDTH>
                    <Frame>
                        <Text style=TextStyle::CAPTION>"Row id prefix"</Text>
                    </Frame>
                </ListItem>
                <ListItem grow=1.0>
                    <TextInput
                        value={Binding::new(prefix, reprefix)}
                        placeholder="BE"
                        disabled={prefix_off}
                        @test_id={"database-view.prefix"}
                    />
                </ListItem>
            </List>
            <Show value={shared}>
                <List spacing=Space::Xs2 @test_id={"database-view.prefix-shared"}>
                    <Text style=TextStyle::CAPTION color=TextColor::Muted>{sharing}</Text>
                    <ForEach value={others}>|other: Uuid| {
                        let editor = data.editor();
                        let found = editor.watch_blocks(BlockQuery::Block(other));
                        let name = create_memo(move || {
                            let types = editor.block_types();
                            found
                                .get()
                                .ok()
                                .and_then(|found| found.into_iter().next())
                                .map_or_else(
                                    || "A database you can't open".to_owned(),
                                    |info| info.label(types.as_ref()).name,
                                )
                        });
                        view! {
                            <Text style=TextStyle::CAPTION>{name}</Text>
                        }
                    }</ForEach>
                </List>
            </Show>
            <List direction=Direction::Horizontal spacing=Space::None>
                <Button
                    glyph=ICON_TABLE_PROPERTIES
                    label="Edit fields"
                    variant=ButtonVariant::Secondary
                    @test_id={"database-view.columns"}
                    on_click={fields_open}
                />
            </List>
        </List>
    }
}

#[component]
fn KindSwitch(data: Data) -> NodeId {
    let keys = create_memo(|| (0..KINDS.len()).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm wrap=true>
            <ForEach value={keys}>|index: usize| {
                let (wanted, label, glyph) = KINDS[index];
                let pressed = create_memo(move || data.kind.get() == wanted);
                let choose = move |_: bool| {
                    if data.kind.get_untracked() != wanted {
                        data.operate_view(DatabaseView::set_kind(wanted));
                    }
                    let first = |wanted: DatabaseFieldType| data.fields.with_untracked(|fields| {
                        fields.iter().find(|field| field.field_type == wanted).map(|field| field.id)
                    });
                    if wanted == DatabaseViewKind::Kanban && data.group_by.get_untracked().is_none() {
                        if let Some(status) = first(DatabaseFieldType::Enum) {
                            data.operate_view(DatabaseView::set_group_by(Some(status)));
                        }
                    }
                    if wanted == DatabaseViewKind::Calendar && data.calendar_start.get_untracked().is_none() {
                        if let Some(date) = first(DatabaseFieldType::Datetime) {
                            data.operate_view(DatabaseView::set_calendar_start(Some(date)));
                        }
                    }
                };
                view! {
                    <ToggleButton
                        label={label}
                        glyph={glyph}
                        pressed={Binding::new(pressed, choose)}
                        @test_id={format!("database-view.kind.{label}")}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn GroupPicker(data: Data) -> NodeId {
    let options = create_memo(move || {
        let board = data.kind.get() == DatabaseViewKind::Kanban;
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| match board {
                    true => field.field_type == DatabaseFieldType::Enum,
                    false => groupable(field),
                })
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    });
    let chosen = data.group_by;
    let picked = move |field: Option<Uuid>| {
        let _ = &options;
        data.operate_view(DatabaseView::set_group_by(field));
    };
    let label = move || match data.kind.get() {
        DatabaseViewKind::Kanban => "Columns".to_owned(),
        _ => "Group by".to_owned(),
    };
    view! {
        <Labelled label={label}>
            <FieldPicker
                options={options}
                chosen={chosen}
                none="No grouping"
                label="Group by"
                picker_id={"database-view.group-by".to_owned()}
                on_change={picked}
            />
        </Labelled>
    }
}

#[component]
fn OrderPicker(data: Data) -> NodeId {
    let options = create_memo(move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| {
                    field.field_type != DatabaseFieldType::Relation
                        || !field.relation_options.multiple
                })
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    });
    let chosen = create_memo(move || data.sort.get().map(|sort| sort.field_id));
    let picked = move |field: Option<Uuid>| {
        let direction = data
            .sort
            .get_untracked()
            .map_or(SortDirection::Ascending, |sort| sort.direction);
        data.operate_view(DatabaseView::set_sort(field.map(|field_id| {
            DatabaseViewSort {
                field_id,
                direction,
            }
        })));
    };
    let flip = move || {
        if let Some(sort) = data.sort.get_untracked() {
            let direction = match sort.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
            data.operate_view(DatabaseView::set_sort(Some(DatabaseViewSort {
                direction,
                ..sort
            })));
        }
    };
    view! {
        <Labelled label="Order by">
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <ListItem grow=1.0>
                    <FieldPicker
                        options={options}
                        chosen={chosen}
                        none="Manual"
                        label="Order by"
                        picker_id={"database-view.order-by".to_owned()}
                        on_change={picked}
                    />
                </ListItem>
                <Show
                    value={move || data.sort.get().map(|sort| sort.direction)}
                >|direction: ReadSignal<SortDirection>|
                    <Button
                        icon_only=true
                        glyph={move || match direction.get() {
                            SortDirection::Ascending => ICON_ARROW_UP,
                            SortDirection::Descending => ICON_ARROW_DOWN,
                        }}
                        label={move || match direction.get() {
                            SortDirection::Ascending => "Ascending".to_owned(),
                            SortDirection::Descending => "Descending".to_owned(),
                        }}
                        @test_id={"database-view.order-direction"}
                        on_click={flip}
                    />
                </Show>
            </List>
        </Labelled>
    }
}

#[component]
fn AxisPickers(data: Data) -> NodeId {
    let options = create_memo(move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.field_type,
                        DatabaseFieldType::Number | DatabaseFieldType::Datetime
                    )
                })
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    });
    let pick_x = move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_scatter_x(field));
    };
    let pick_y = move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_scatter_y(field));
    };
    let (x_chosen, y_chosen) = (data.scatter_x, data.scatter_y);
    view! {
        <List spacing=SPACING>
            <Labelled label="X axis">
                <FieldPicker
                    options={options}
                    chosen={x_chosen}
                    none="None"
                    label="X axis"
                    picker_id={"database-view.scatter-x".to_owned()}
                    on_change={pick_x}
                />
            </Labelled>
            <Labelled label="Y axis">
                <FieldPicker
                    options={options}
                    chosen={y_chosen}
                    none="None"
                    label="Y axis"
                    picker_id={"database-view.scatter-y".to_owned()}
                    on_change={pick_y}
                />
            </Labelled>
        </List>
    }
}

#[component]
fn DatePickers(data: Data) -> NodeId {
    let options = create_memo(move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type == DatabaseFieldType::Datetime)
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    });
    let pick_start = move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_calendar_start(field));
    };
    let pick_end = move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_calendar_end(field));
    };
    view! {
        <List spacing=SPACING>
            <Labelled label="Starts">
                <FieldPicker
                    options={options}
                    chosen={data.calendar_start}
                    none="None"
                    label="Starts"
                    picker_id={"database-view.calendar-start".to_owned()}
                    on_change={pick_start}
                />
            </Labelled>
            <Labelled label="Ends">
                <FieldPicker
                    options={options}
                    chosen={data.calendar_end}
                    none="None"
                    label="Ends"
                    picker_id={"database-view.calendar-end".to_owned()}
                    on_change={pick_end}
                />
            </Labelled>
        </List>
    }
}

#[component]
fn EmptyGroups(data: Data) -> NodeId {
    let on = data.show_empty_groups;
    let toggle = move |on: bool| data.operate_view(DatabaseView::set_show_empty_groups(on));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <ListItem grow=1.0>
                <Text>"Show empty groups"</Text>
            </ListItem>
            <Switch
                on={Binding::new(on, toggle)}
                label="Show empty groups"
                @test_id={"database-view.empty-groups"}
            />
        </List>
    }
}

#[component]
fn ShownFields(data: Data) -> NodeId {
    let keys = create_memo(move || {
        let title = data.title_field.get();
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| Some(field.id) != title)
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    });
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::CAPTION>"Shown on each row"</Text>
            <List direction=Direction::Horizontal spacing=Space::Sm wrap=true>
                <ForEach value={keys}>|field: Uuid| {
                    let name = create_memo(move || {
                        data.fields.with(|fields| {
                            fields.iter().find(|known| known.id == field).map_or_else(String::new, |known| known.name.clone())
                        })
                    });
                    let pressed = create_memo(move || {
                        data.hidden_fields.with(|hidden| !hidden.contains(&field))
                    });
                    let toggle = move |shown: bool| {
                        let mut hidden = data.hidden_fields.get_untracked();
                        hidden.retain(|held| *held != field);
                        if !shown {
                            hidden.push(field);
                        }
                        data.operate_view(DatabaseView::set_hidden_fields(hidden));
                    };
                    view! {
                        <ToggleButton
                            label={name}
                            pressed={Binding::new(pressed, toggle)}
                            @test_id={format!("database-view.shown.{field}")}
                        />
                    }
                }</ForEach>
            </List>
        </List>
    }
}

#[component]
fn Labelled(
    label: be_sdk_beui::beui::reactive::Prop<String>,
    children: be_sdk_beui::beui::reactive::Child,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <ListItem length=LABEL_WIDTH>
                <Frame>
                    <Text style=TextStyle::CAPTION>{label}</Text>
                </Frame>
            </ListItem>
            <ListItem grow=1.0>
                <Frame>{children}</Frame>
            </ListItem>
        </List>
    }
}

#[component]
pub fn FieldPicker(
    options: Memo<Vec<DatabaseField>>,
    chosen: Memo<Option<Uuid>>,
    none: &'static str,
    label: &'static str,
    picker_id: String,
    on_change: Callback<Option<Uuid>>,
) -> NodeId {
    let selected = create_memo(move || Some(chosen.get()));
    let picked = move |field: Option<Option<Uuid>>| {
        if let Some(field) = field {
            on_change.call(field);
        }
    };
    view! {
        <Select
            options={view! {
                <ChoiceOption value=None label=none />
                <ForEach
                    value={options}
                    key={|field: &DatabaseField| field.id}
                >|field: ReadSignal<DatabaseField>| {
                    let name = create_memo(move || field.with(|field| field.name.clone()));
                    let id = field.with_untracked(|field| field.id);
                    view! {
                        <ChoiceOption value={Some(id)} label={name} />
                    }
                }</ForEach>
            }}
            selected={Binding::new(selected, picked)}
            label={label}
            @test_id={picker_id}
        />
    }
}
