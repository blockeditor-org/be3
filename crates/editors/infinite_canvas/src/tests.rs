use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::database::DatabaseValue;
use block_client::blocks::infinite_canvas::{
    CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint,
    CanvasPreviewRegion, CanvasTransform, InfiniteCanvas, InfiniteCanvasOperation,
};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CanvasApp;
use crate::app::state::{attach_component, remove_component, set_component_value};
use crate::geometry::{ResizeHandle, entity_bounds, resize_entities_axis};

mod a_direct_editor_entity_draws_the_frame_it_reserves;
mod attaching_component_fills_only_missing_selected_entities;
mod clicking_an_entity_selects_it_and_shows_its_handles;
mod dragging_with_the_rectangle_tool_adds_a_rectangle;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod the_actions_menu_deletes_the_selection;
mod the_canvas_paints_the_entities_it_holds;
mod the_intrinsic_size_follows_the_preview_region;
mod the_preview_centres_the_region_it_was_given;

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

fn editor(entities: &[CanvasEntity]) -> (BeuiTest<CanvasApp>, BlockHandle<InfiniteCanvas>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(InfiniteCanvas::new());
    for entity in entities {
        block.operate(InfiniteCanvasOperation::Add {
            entity: entity.clone(),
        });
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.run();
    editor.run();
    (editor, block)
}

fn entities(block: &BlockHandle<InfiniteCanvas>) -> Vec<CanvasEntity> {
    block
        .read()
        .expect("the canvas is loaded")
        .entities()
        .to_vec()
}
