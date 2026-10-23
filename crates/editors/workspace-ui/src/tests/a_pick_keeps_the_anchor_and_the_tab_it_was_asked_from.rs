use block_editor_beui::beui::styled::Root;
use block_editor_beui::beui::{Pos2, Vec2};
use std::cell::Cell;
use std::rc::Rc;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{pos2, vec2};
use block_editor_beui::{BlockLocation, Rect, ShowRequest};
use block_shell::{PickAnchor, Workspace};

use super::*;
use crate::app::WorkspaceBody;

#[test]
fn a_pick_keeps_the_anchor_and_the_tab_it_was_asked_from() {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let owner = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let editor = *owner;
    let held = Rc::new(Cell::new(None::<Workspace>));
    let keep = Rc::clone(&held);
    let mut test = BeuiTest::<WorkspaceUiApp>::with_view(owner, move || {
        let workspace = Workspace::new(editor, true, false);
        keep.set(Some(workspace));
        view! {
            <Root>
                <WorkspaceBody workspace />
            </Root>
        }
    });
    test.run();
    let workspace = held.get().expect("the workspace is built");

    let notes = Uuid::new_v4();
    host.show_block(ShowRequest {
        block_id: notes,
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    test.run();
    let tab = workspace.layout().shown();
    assert!(tab.is_some(), "the shown block has a tab");

    let mut asked = filter();
    asked.anchor = Some(Rect::from_min_size(
        Pos2::new(412.0, 96.0),
        Vec2::new(28.0, 28.0),
    ));
    let inner = Uuid::new_v4();
    test.request_pick(
        9,
        asked,
        BlockLocation::Block(inner.into_bytes()),
        Some(notes),
    );
    test.request_pick(10, filter(), BlockLocation::Root, None);
    test.run();

    assert_eq!(
        workspace.pick_anchor(9),
        Some(PickAnchor {
            rect: Some(Rect::from_min_size(pos2(412.0, 96.0), vec2(28.0, 28.0))),
            tab,
        }),
        "a block inside a tab asked from its button"
    );
    assert_eq!(
        workspace.pick_anchor(10),
        Some(PickAnchor {
            rect: None,
            tab: None,
        }),
        "a creation dialog asks from no tab and no button"
    );
}
