use block_editor_plugin::be_block::BlockContent;
use std::rc::Rc;

use block_editor_plugin::be_block::database::DatabaseValue;
use block_editor_plugin::be_block::database_schema::DatabaseSchemaContent;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Show, Spacer, clone, component, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::styled::{Accordion, Button, ButtonVariant, Caption, Separator};
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::block_ui::database::{DatabaseBlockPickRequest, DatabaseValueChange};
use block_editor_plugin::database::{DatabaseValueEditor, RowValues, ValueLabels};
use uuid::Uuid;

use super::state::{CanvasState, attach_component, remove_component, set_component_value};

const SPACING: f32 = 10.0;

#[component]
pub(crate) fn CanvasComponents(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let schemas = create_memo(clone!(state -> move || {
        let mut seen = Vec::new();
        for entity in state.selected_entities() {
            for component in &entity.components {
                if !seen.contains(&component.schema_id) {
                    seen.push(component.schema_id);
                }
            }
        }
        seen
    }));
    let rows = Rc::clone(&state);
    let add = clone!(state -> move || state.open_component_picker());
    view! {
        <Accordion title="Components" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=SPACING>
                <ForEach keys={schemas}>
                    {move |schema_id: Uuid| {
                        let state = Rc::clone(&rows);
                        view! {
                            <ComponentRow state schema_id />
                        }
                    }}
                </ForEach>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Button
                        label="Add component…"
                        variant=ButtonVariant::Secondary
                        @test_id={"infinite-canvas.add-component"}
                        on_click={add}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Accordion>
    }
}

#[component]
fn ComponentRow(state: Rc<CanvasState>, schema_id: Uuid) -> NodeId {
    let schema = state
        .editor()
        .related_content::<DatabaseSchemaContent>(create_memo(move || Some(schema_id)));
    let fields = schema.project(|schema| schema.root().fields());
    let fields = create_memo(clone!(fields -> move || fields.get()));
    let named = create_memo(clone!(state -> move || {
        state
            .label_of(schema_id)
            .map(|label: BlockLabel| label.name)
            .unwrap_or_else(|| "Component".to_owned())
    }));
    let attached = create_memo(clone!(state -> move || {
        state
            .selected_entities()
            .iter()
            .filter(|entity| {
                entity
                    .components
                    .iter()
                    .any(|component| component.schema_id == schema_id)
            })
            .count()
    }));
    let selected = create_memo(clone!(state -> move || state.selected_entities().len()));
    let partial =
        create_memo(clone!(attached selected -> move || attached.get() != selected.get()));
    let coverage = create_memo(clone!(attached selected -> move || {
        format!("Attached to {} of {}", attached.get(), selected.get())
    }));
    let whole = create_memo(clone!(partial -> move || !partial.get()));
    let values = create_memo(clone!(state -> move || {
        let mut shared: Option<RowValues> = None;
        for entity in state.selected_entities() {
            let Some(component) = entity
                .components
                .iter()
                .find(|component| component.schema_id == schema_id)
            else {
                continue;
            };
            shared = Some(match shared {
                None => component.values.clone(),
                Some(held) => held
                    .into_iter()
                    .filter(|(field, value)| {
                        component.values.get(field).is_some_and(|held| held == value)
                    })
                    .collect(),
            });
        }
        shared.unwrap_or_default()
    }));
    let labels = create_memo(clone!(state -> move || {
        let mut labels = ValueLabels::new();
        for entity in state.selected_entities() {
            for component in &entity.components {
                for value in component.values.values() {
                    if let DatabaseValue::Block(reference) = value
                        && let Some(label) = state.label_of(*reference)
                    {
                        labels.insert(*reference, label);
                    }
                }
            }
        }
        labels
    }));
    let read_only = state.editor().read_only();
    let adding = Rc::clone(&state);
    let removing = Rc::clone(&state);
    let opening = Rc::clone(&state);
    let changing = Rc::clone(&state);
    let picking = Rc::clone(&state);
    let add_all = clone!(adding -> move || {
        adding.edit_components(|entities, selected| {
            attach_component(entities, selected, schema_id);
        });
    });
    let remove = clone!(removing -> move || {
        removing.edit_components(|entities, selected| {
            remove_component(entities, selected, schema_id);
        });
    });
    let open_schema = clone!(opening -> move || {
        opening
            .editor()
            .host()
            .open_block(schema_id, DatabaseSchemaContent::CONTENT_TYPE);
    });
    let changed = clone!(changing -> move |change: DatabaseValueChange| {
        changing.edit_components(|entities, selected| {
            set_component_value(entities, selected, schema_id, change.field_id, change.value);
        });
    });
    let picked = clone!(picking -> move |request: DatabaseBlockPickRequest| {
        picking.open_value_picker(schema_id, request.field_id, request.block_type);
    });
    let prefix = format!("infinite-canvas.component.{schema_id:?}");
    let add_id = format!("{prefix}.add-to-all");
    let open_id = format!("{prefix}.open");
    let remove_id = format!("{prefix}.remove");
    view! {
        <List spacing=6.0>
            <Caption content={named} />
            <Show condition={partial.clone()}>
                <List spacing=6.0>
                    <Caption content={coverage} />
                    <List direction=Direction::Horizontal spacing=4.0>
                        <Button
                            label="Add to all"
                            variant=ButtonVariant::Secondary
                            @test_id={add_id}
                            on_click={add_all}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                </List>
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                <Button
                    label="Open schema"
                    variant=ButtonVariant::Secondary
                    @test_id={open_id}
                    on_click={open_schema}
                />
                <Button
                    label="Remove"
                    variant=ButtonVariant::Secondary
                    @test_id={remove_id}
                    on_click={remove}
                />
            </List>
            <Show condition={whole}>
                <DatabaseValueEditor
                    fields={fields}
                    values={values}
                    labels={labels}
                    disabled={read_only}
                    prefix={prefix}
                    on_change={changed}
                    on_pick={picked}
                />
            </Show>
            <Separator />
        </List>
    }
}
