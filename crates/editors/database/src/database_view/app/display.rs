use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CHART_SCATTER, ICON_GRID_3X3, ICON_KANBAN, ICON_LIST,
    ICON_TABLE_PROPERTIES,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use database_types::database_schema::{DatabaseField, DatabaseFieldType};
use database_types::database_view::{
    DatabaseView, DatabaseViewKind, DatabaseViewSort, SortDirection,
};
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::groups::groupable;

const SPACING: Space = Space::Md;
const LABEL_WIDTH: f32 = 96.0;
pub const KINDS: [(DatabaseViewKind, &str, Glyph); 4] = [
    (DatabaseViewKind::List, "List", ICON_LIST),
    (DatabaseViewKind::Kanban, "Board", ICON_KANBAN),
    (DatabaseViewKind::Spreadsheet, "Table", ICON_GRID_3X3),
    (DatabaseViewKind::Scatter, "Scatter", ICON_CHART_SCATTER),
];

#[component]
pub fn DisplayOptions(data: Data) -> NodeId {
    let listed = create_memo(move || data.kind.get() == DatabaseViewKind::List);
    let grouped = create_memo(move || {
        matches!(
            data.kind.get(),
            DatabaseViewKind::List | DatabaseViewKind::Kanban
        )
    });
    let ordered = create_memo(move || data.kind.get() != DatabaseViewKind::Scatter);
    let scatter = create_memo(move || data.kind.get() == DatabaseViewKind::Scatter);
    let prefix = data.prefix;
    let prefix_off = data.read_only;
    let reprefix = move |text: String| data.set_prefix(text);
    let fields_open = move || data.open_schema();
    view! {
        <List spacing=SPACING>
            <KindSwitch data={data} />
            <Separator />
            <Show condition={grouped}>
                <GroupPicker data={data} />
            </Show>
            <Show condition={ordered}>
                <OrderPicker data={data} />
            </Show>
            <Show condition={scatter}>
                <AxisPickers data={data} />
            </Show>
            <Show condition={listed}>
                <EmptyGroups data={data} />
            </Show>
            <Show condition={listed}>
                <List spacing=SPACING>
                    <Separator />
                    <ShownFields data={data} />
                </List>
            </Show>
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Frame @sizing=ItemSize::Fixed(LABEL_WIDTH)>
                    <Text style=TextStyle::CAPTION>"Row id prefix"</Text>
                </Frame>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(prefix, reprefix)}
                    placeholder="BE"
                    disabled={prefix_off}
                    @test_id={"database-view.prefix"}
                />
            </List>
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
            <ForEach keys={keys}>|index: usize| {
                let (wanted, label, glyph) = KINDS[index];
                let pressed = create_memo(move || data.kind.get() == wanted);
                let choose = move |_: bool| {
                    if data.kind.get_untracked() != wanted {
                        data.operate_view(DatabaseView::set_kind(wanted));
                    }
                    if wanted == DatabaseViewKind::Kanban && data.group_by.get_untracked().is_none() {
                        let status = data.fields.with_untracked(|fields| {
                            fields.iter().find(|field| field.field_type == DatabaseFieldType::Enum).map(|field| field.id)
                        });
                        if status.is_some() {
                            data.operate_view(DatabaseView::set_group_by(status));
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
    let label = create_memo(move || match data.kind.get() {
        DatabaseViewKind::Kanban => "Columns".to_owned(),
        _ => "Group by".to_owned(),
    });
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
    let sorted = create_memo(move || data.sort.get().is_some());
    let glyph = create_memo(move || {
        match data.sort.get().map(|sort| sort.direction) {
            Some(SortDirection::Descending) => ICON_ARROW_DOWN,
            _ => ICON_ARROW_UP,
        }
        .to_owned()
    });
    let direction_label = create_memo(move || {
        match data.sort.get().map(|sort| sort.direction) {
            Some(SortDirection::Descending) => "Descending",
            _ => "Ascending",
        }
        .to_owned()
    });
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
                <FieldPicker
                    @sizing=ItemSize::Percent(100.0)
                    options={options}
                    chosen={chosen}
                    none="Manual"
                    label="Order by"
                    picker_id={"database-view.order-by".to_owned()}
                    on_change={picked}
                />
                <Show condition={sorted}>
                    <Button
                        icon_only=true
                        glyph={glyph}
                        label={direction_label}
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
fn EmptyGroups(data: Data) -> NodeId {
    let on = data.show_empty_groups;
    let toggle = move |on: bool| data.operate_view(DatabaseView::set_show_empty_groups(on));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Text @sizing=ItemSize::Percent(100.0)>"Show empty groups"</Text>
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
                <ForEach keys={keys}>|field: Uuid| {
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
    label: block_editor_beui::beui::reactive::Prop<String>,
    children: block_editor_beui::beui::reactive::Child,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Frame @sizing=ItemSize::Fixed(LABEL_WIDTH)>
                <Text style=TextStyle::CAPTION>{label}</Text>
            </Frame>
            <Frame @sizing=ItemSize::Percent(100.0)>{children}</Frame>
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
    let keys =
        create_memo(move || options.with(|options| (0..=options.len()).collect::<Vec<usize>>()));
    let selected = create_memo(move || match chosen.get() {
        None => Some(0),
        Some(chosen) => options
            .with(|options| options.iter().position(|field| field.id == chosen))
            .map(|index| index + 1),
    });
    let picked = move |index: Option<usize>| {
        let field = match index {
            None | Some(0) => None,
            Some(index) => {
                options.with_untracked(|options| options.get(index - 1).map(|field| field.id))
            }
        };
        on_change.call(field);
    };
    view! {
        <Select
            options={view! {
                <ForEach keys={keys}>|index: usize| {
                    let name = create_memo(move || match index {
                        0 => none.to_owned(),
                        index => options.with(|options| {
                            options.get(index - 1).map_or_else(String::new, |field| field.name.clone())
                        }),
                    });
                    view! {
                        <ChoiceOption label={name} />
                    }
                }</ForEach>
            }}
            selected={Binding::new(selected, picked)}
            label={label}
            @test_id={picker_id}
        />
    }
}
