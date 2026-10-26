use block_editor_beui::be_block::BlockContent;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::block_ui::database::DatabaseValueChange;
use block_editor_beui::database::DatabaseBlockPickRequest;
use block_editor_beui::database::{DatabaseValueEditor, RowValues, ValueLabels};
use block_editor_beui::node_anchor;
use database_types::database::DatabaseValue;
use database_types::database_schema::DatabaseSchemaContent;
use uuid::Uuid;

use super::state::{CanvasState, attach_component, remove_component, set_component_value};

const SPACING: Space = Space::Md;

#[component]
pub(crate) fn CanvasComponents(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let schemas = create_memo(move || {
        let mut seen = Vec::new();
        for entity in state.selected_entities() {
            for component in &entity.components {
                if !seen.contains(&component.schema_id) {
                    seen.push(component.schema_id);
                }
            }
        }
        seen
    });
    let add_button = NodeRef::new();
    let add = move || state.open_component_picker(node_anchor(&add_button));
    view! {
        <Accordion title="Components" open={open}>
            <List spacing=SPACING>
                <ForEach keys={schemas}>|schema_id: Uuid|
                    <ComponentRow state schema_id />
                </ForEach>
                <List direction=Direction::Horizontal spacing=Space::None>
                    <Button
                        @node_ref={&add_button}
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
fn ComponentRow(state: CanvasState, schema_id: Uuid) -> NodeId {
    let schema = state
        .editor()
        .related_content::<DatabaseSchemaContent>(create_memo(move || Some(schema_id)));
    let fields = schema.project(|schema| schema.root().fields());
    let fields = create_memo(move || fields.get());
    let named = create_memo(move || {
        state
            .label_of(schema_id)
            .map(|label: BlockLabel| label.name)
            .unwrap_or_else(|| "Component".to_owned())
    });
    let attached = create_memo(move || {
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
    });
    let selected = create_memo(move || state.selected_entities().len());
    let partial = create_memo(move || attached.get() != selected.get());
    let coverage =
        create_memo(move || format!("Attached to {} of {}", attached.get(), selected.get()));
    let whole = create_memo(move || !partial.get());
    let values = create_memo(move || {
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
                        component
                            .values
                            .get(field)
                            .is_some_and(|held| held == value)
                    })
                    .collect(),
            });
        }
        shared.unwrap_or_default()
    });
    let labels = create_memo(move || {
        let mut labels = ValueLabels::new();
        for entity in state.selected_entities() {
            for component in &entity.components {
                for value in component.values.values() {
                    if let DatabaseValue::Block(reference) = value
                        && let Some(label) = state.label_of(*reference)
                    {
                        labels.blocks.insert(*reference, label);
                    }
                }
            }
        }
        labels
    });
    let read_only = state.editor().read_only();
    let add_all = move || {
        state.edit_components(|entities, selected| {
            attach_component(entities, selected, schema_id);
        });
    };
    let remove = move || {
        state.edit_components(|entities, selected| {
            remove_component(entities, selected, schema_id);
        });
    };
    let open_schema = move || {
        state
            .editor()
            .host()
            .open_block(schema_id, DatabaseSchemaContent::CONTENT_TYPE);
    };
    let changed = move |change: DatabaseValueChange| {
        state.edit_components(|entities, selected| {
            set_component_value(entities, selected, schema_id, change.field_id, change.value);
        });
    };
    let picked = move |request: DatabaseBlockPickRequest| {
        state.open_value_picker(schema_id, request);
    };
    let prefix = format!("infinite-canvas.component.{schema_id:?}");
    let add_id = format!("{prefix}.add-to-all");
    let open_id = format!("{prefix}.open");
    let remove_id = format!("{prefix}.remove");
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::CAPTION>{named}</Text>
            <Show condition={partial}>|| {
                let add_id = add_id.clone(); view! {
                    <List spacing=Space::Sm>
                        <Text style=TextStyle::CAPTION>{coverage}</Text>
                        <List direction=Direction::Horizontal spacing=Space::Xs>
                            <Button
                                label="Add to all"
                                variant=ButtonVariant::Secondary
                                @test_id={add_id}
                                on_click={add_all}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        </List>
                    </List>
                }
            }</Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs wrap=true>
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
                    prefix={prefix.clone()}
                    on_change={changed}
                    on_pick={picked}
                />
            </Show>
            <Separator />
        </List>
    }
}
