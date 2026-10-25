use super::*;
use block_editor_beui::beui::styled::Root;
use std::cell::RefCell;
use std::collections::HashMap;

use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use infinite_canvas_types::CanvasColor;

use crate::app::actions::canvas_actions;
use crate::app::canvas::CanvasStage;
use crate::app::state::CanvasState;

thread_local! {
    static WOKEN: RefCell<HashMap<Uuid, usize>> = RefCell::new(HashMap::new());
}

#[component]
fn CountingCanvas(editor: block_editor_beui::Editor, ids: Vec<Uuid>) -> NodeId {
    let state = CanvasState::new(editor, false);
    state.watch();
    provide_context(canvas_actions(state));
    for id in ids {
        let signals = state.entities.watch(id);
        let stored = signals.stored();
        create_effect(move || {
            stored.with(|_| ());
            WOKEN.with_borrow_mut(|woken| *woken.entry(id).or_default() += 1);
        });
    }
    view! {
        <CanvasStage state={state} />
    }
}

#[test]
fn dragging_one_entity_wakes_only_that_entity() {
    let entities: Vec<CanvasEntity> = (1..=3)
        .map(|index| {
            let mut entity = entity(Uuid::from_u128(index));
            entity.transform = CanvasTransform::new(
                CanvasPoint::new(index as f32 * 200.0 - 400.0, 0.0),
                CanvasPoint::new(120.0, 80.0),
                0.0,
            );
            entity.style.fill = CanvasFill::Solid;
            entity.style.fill_color = CanvasColor::Rgba {
                red: 60,
                green: 110,
                blue: 90,
                alpha: 255,
            };
            entity
        })
        .collect();
    let ids: Vec<Uuid> = entities.iter().map(|entity| entity.id).collect();
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host, Uuid::new_v4());
    let handle = *owner;
    let watched = ids.clone();
    let mut editor = BeuiTest::<CanvasApp>::with_view(owner, move || {
        let ids = watched.clone();
        view! {
            <Root>
                <Loaded>
                    <CountingCanvas editor={handle} ids={ids} />
                </Loaded>
            </Root>
        }
    })
    .in_viewport();
    editor.hold(None, CanvasContent::new(&Canvas::with_entities(entities)));
    editor.run();
    WOKEN.with_borrow_mut(HashMap::clear);

    let moved = ids[1];
    let from = editor
        .rect_of(&format!("infinite-canvas.entity.{moved}"))
        .center();
    editor.drag(from, from + Vec2::new(60.0, 40.0));
    editor.run();

    let after = super::entities(&editor);
    let dropped = after.iter().find(|entity| entity.id == moved).unwrap();
    assert!(
        dropped.transform.center.x > 0.0,
        "the drag did not move the entity: {dropped:?}"
    );
    let woken = WOKEN.with_borrow(Clone::clone);
    let stored = woken.get(&moved).copied().unwrap_or_default();
    assert!(
        stored >= 2,
        "the dragged entity was written {stored} times, not as it moved"
    );
    for id in ids.iter().filter(|id| **id != moved) {
        assert_eq!(
            woken.get(id),
            None,
            "dragging {moved} woke the signals of {id}"
        );
    }
}
