use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::be_block::database_view::{
    DatabaseView, DatabaseViewKind, DatabaseViewSort, SortDirection,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_GRID_ON, ICON_SCATTER_PLOT, ICON_SCHEMA,
    ICON_VIEW_KANBAN, ICON_VIEW_LIST,
};
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconButton, Select, Separator, Switch, TextInput,
    ToggleButton,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::groups::groupable;

const SPACING: f32 = 10.0;
const LABEL_WIDTH: f32 = 96.0;
pub const KINDS: [(DatabaseViewKind, &str, &str); 4] = [
    (DatabaseViewKind::List, "List", ICON_VIEW_LIST),
    (DatabaseViewKind::Kanban, "Board", ICON_VIEW_KANBAN),
    (DatabaseViewKind::Spreadsheet, "Table", ICON_GRID_ON),
    (DatabaseViewKind::Scatter, "Scatter", ICON_SCATTER_PLOT),
];

#[component]
pub fn DisplayOptions(data: Data) -> NodeId {
    let listed = create_memo(clone!(data -> move || data.kind.get() == DatabaseViewKind::List));
    let grouped = create_memo(clone!(data -> move || {
        matches!(data.kind.get(), DatabaseViewKind::List | DatabaseViewKind::Kanban)
    }));
    let ordered = create_memo(clone!(data -> move || data.kind.get() != DatabaseViewKind::Scatter));
    let scatter = create_memo(clone!(data -> move || data.kind.get() == DatabaseViewKind::Scatter));
    let prefix = data.prefix.clone();
    let prefix_off = data.read_only.clone();
    let reprefix = clone!(data -> move |text: String| data.set_prefix(text));
    let fields_open = clone!(data -> move || data.open_schema());
    let (switch, groups, order, axes, empty, shown) = (
        data.clone(),
        data.clone(),
        data.clone(),
        data.clone(),
        data.clone(),
        data.clone(),
    );
    view! {
        <List spacing=SPACING>
            <KindSwitch data={switch} />
            <Separator />
            <Show condition={grouped}>
                {move || clone!(groups -> view! { <GroupPicker data={groups} /> })}
            </Show>
            <Show condition={ordered}>
                {move || clone!(order -> view! { <OrderPicker data={order} /> })}
            </Show>
            <Show condition={scatter}>
                {move || clone!(axes -> view! { <AxisPickers data={axes} /> })}
            </Show>
            <Show condition={listed.clone()}>
                {move || clone!(empty -> view! { <EmptyGroups data={empty} /> })}
            </Show>
            <Show condition={listed}>
                {move || clone!(shown -> view! {
                    <List spacing=SPACING>
                        <Separator />
                        <ShownFields data={shown} />
                    </List>
                })}
            </Show>
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Frame @sizing=ItemSize::Fixed(LABEL_WIDTH)>
                    <Caption content="Row id prefix" />
                </Frame>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={prefix}
                    placeholder="BE"
                    disabled={prefix_off}
                    @test_id={"database-view.prefix"}
                    on_change={reprefix}
                />
            </List>
            <List direction=Direction::Horizontal spacing=0.0>
                <Button
                    glyph={ICON_SCHEMA.to_owned()}
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
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0 wrap=true>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let (wanted, label, glyph) = KINDS[index];
                    let data = data.clone();
                    let pressed = create_memo(clone!(data -> move || data.kind.get() == wanted));
                    let choose = clone!(data -> move |_: bool| {
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
                    });
                    view! {
                        <ToggleButton
                            label={label}
                            glyph={glyph.to_owned()}
                            pressed={pressed}
                            @test_id={format!("database-view.kind.{label}")}
                            on_change={choose}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn GroupPicker(data: Data) -> NodeId {
    let options = create_memo(clone!(data -> move || {
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
    }));
    let chosen = data.group_by.clone();
    let picked = clone!(data options -> move |field: Option<Uuid>| {
        let _ = &options;
        data.operate_view(DatabaseView::set_group_by(field));
    });
    let label = create_memo(clone!(data -> move || match data.kind.get() {
        DatabaseViewKind::Kanban => "Columns".to_owned(),
        _ => "Group by".to_owned(),
    }));
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
    let options = create_memo(clone!(data -> move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type != DatabaseFieldType::Relation || !field.relation_options.multiple)
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    }));
    let chosen = create_memo(clone!(data -> move || data.sort.get().map(|sort| sort.field_id)));
    let picked = clone!(data -> move |field: Option<Uuid>| {
        let direction = data
            .sort
            .get_untracked()
            .map_or(SortDirection::Ascending, |sort| sort.direction);
        data.operate_view(DatabaseView::set_sort(field.map(|field_id| DatabaseViewSort {
            field_id,
            direction,
        })));
    });
    let sorted = create_memo(clone!(data -> move || data.sort.get().is_some()));
    let glyph = create_memo(clone!(data -> move || {
        match data.sort.get().map(|sort| sort.direction) {
            Some(SortDirection::Descending) => ICON_ARROW_DOWNWARD,
            _ => ICON_ARROW_UPWARD,
        }
        .to_owned()
    }));
    let direction_label = create_memo(clone!(data -> move || {
        match data.sort.get().map(|sort| sort.direction) {
            Some(SortDirection::Descending) => "Descending",
            _ => "Ascending",
        }
        .to_owned()
    }));
    let flip = clone!(data -> move || {
        if let Some(sort) = data.sort.get_untracked() {
            let direction = match sort.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
            data.operate_view(DatabaseView::set_sort(Some(DatabaseViewSort { direction, ..sort })));
        }
    });
    view! {
        <Labelled label="Order by">
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
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
                    <IconButton
                        glyph={glyph.clone()}
                        label={direction_label.clone()}
                        @test_id={"database-view.order-direction"}
                        on_click={flip.clone()}
                    />
                </Show>
            </List>
        </Labelled>
    }
}

#[component]
fn AxisPickers(data: Data) -> NodeId {
    let options = create_memo(clone!(data -> move || {
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| matches!(field.field_type, DatabaseFieldType::Number | DatabaseFieldType::Datetime))
                .cloned()
                .collect::<Vec<DatabaseField>>()
        })
    }));
    let pick_x = clone!(data -> move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_scatter_x(field));
    });
    let pick_y = clone!(data -> move |field: Option<Uuid>| {
        data.operate_view(DatabaseView::set_scatter_y(field));
    });
    let (x_options, y_options) = (options.clone(), options);
    let (x_chosen, y_chosen) = (data.scatter_x.clone(), data.scatter_y.clone());
    view! {
        <List spacing=SPACING>
            <Labelled label="X axis">
                <FieldPicker
                    options={x_options}
                    chosen={x_chosen.clone()}
                    none="None"
                    label="X axis"
                    picker_id={"database-view.scatter-x".to_owned()}
                    on_change={pick_x}
                />
            </Labelled>
            <Labelled label="Y axis">
                <FieldPicker
                    options={y_options}
                    chosen={y_chosen.clone()}
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
    let on = data.show_empty_groups.clone();
    let toggle = clone!(data -> move |on: bool| data.operate_view(DatabaseView::set_show_empty_groups(on)));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Body @sizing=ItemSize::Percent(100.0) content="Show empty groups" />
            <Switch
                on={on}
                label="Show empty groups"
                @test_id={"database-view.empty-groups"}
                on_change={toggle}
            />
        </List>
    }
}

#[component]
fn ShownFields(data: Data) -> NodeId {
    let keys = create_memo(clone!(data -> move || {
        let title = data.title_field.get();
        data.fields.with(|fields| {
            fields
                .iter()
                .filter(|field| Some(field.id) != title)
                .map(|field| field.id)
                .collect::<Vec<Uuid>>()
        })
    }));
    view! {
        <List spacing=6.0>
            <Caption content="Shown on each row" />
            <List direction=Direction::Horizontal spacing=6.0 wrap=true>
                <ForEach keys={keys}>
                    {move |field: Uuid| {
                        let data = data.clone();
                        let name = create_memo(clone!(data -> move || {
                            data.fields.with(|fields| {
                                fields.iter().find(|known| known.id == field).map_or_else(String::new, |known| known.name.clone())
                            })
                        }));
                        let pressed = create_memo(clone!(data -> move || {
                            data.hidden_fields.with(|hidden| !hidden.contains(&field))
                        }));
                        let toggle = clone!(data -> move |shown: bool| {
                            let mut hidden = data.hidden_fields.get_untracked();
                            hidden.retain(|held| *held != field);
                            if !shown {
                                hidden.push(field);
                            }
                            data.operate_view(DatabaseView::set_hidden_fields(hidden));
                        });
                        view! {
                            <ToggleButton
                                label={name}
                                pressed={pressed}
                                @test_id={format!("database-view.shown.{field}")}
                                on_change={toggle}
                            />
                        }
                    }}
                </ForEach>
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
                <Caption content={label} />
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
    let keys = create_memo(clone!(options -> move || {
        options.with(|options| (0..=options.len()).collect::<Vec<usize>>())
    }));
    let selected = create_memo(clone!(options chosen -> move || {
        match chosen.get() {
            None => Some(0),
            Some(chosen) => options
                .with(|options| options.iter().position(|field| field.id == chosen))
                .map(|index| index + 1),
        }
    }));
    let names = options.clone();
    let picked = clone!(options -> move |index: Option<usize>| {
        let field = match index {
            None | Some(0) => None,
            Some(index) => options.with_untracked(|options| options.get(index - 1).map(|field| field.id)),
        };
        on_change.call(field);
    });
    view! {
        <Select
            options={view! {
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let name = create_memo(clone!(names -> move || match index {
                            0 => none.to_owned(),
                            index => names.with(|options| {
                                options.get(index - 1).map_or_else(String::new, |field| field.name.clone())
                            }),
                        }));
                        view! {
                            <ChoiceOption label={name} />
                        }
                    }}
                </ForEach>
            }}
            selected={selected}
            label={label}
            @test_id={picker_id}
            on_change={picked}
        />
    }
}
