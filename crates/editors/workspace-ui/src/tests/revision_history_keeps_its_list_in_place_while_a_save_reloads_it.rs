use super::*;
use block_editor_beui::{BlockHistory, Revision};
use block_ui_test::{HostCall, Reply};

fn listing(
    fixture: &mut Fixture,
    block: Uuid,
) -> Option<Reply<Result<Vec<Revision>, block_editor_beui::Failure>>> {
    fixture
        .test
        .take_requests()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            HostCall::ListRevisions {
                reply,
                block: asked,
            } if asked == block.into_bytes() => Some(reply),
            _ => None,
        })
}

fn button(fixture: &Fixture, commit: u8) -> Rect {
    let document = fixture.test.document();
    let node = document
        .find_test_id(&format!(
            "workspace.revision.{}",
            format!("{commit:02x}").repeat(32)
        ))
        .expect("the revision is listed");
    document.node_rect(node).expect("the revision is laid out")
}

#[test]
fn revision_history_keeps_its_list_in_place_while_a_save_reloads_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
    show(&mut fixture, opened, None);
    let request = listing(&mut fixture, opened).expect("the active file requests its history");
    fixture.test.answer(
        request,
        Ok(vec![Revision {
            commit: [1; 32],
            time: 1_800_000_000_000,
            current: true,
        }]),
    );
    fixture.settle();
    fixture.show_history();
    let listed = button(&fixture, 1);

    fixture.test.set_histories([(
        opened,
        BlockHistory {
            head: Some([2; 32]),
            can_undo: true,
            can_redo: false,
        },
    )]);
    fixture.settle();
    let request = listing(&mut fixture, opened).expect("a save lists the history again");
    assert!(!fixture.says("Loading revisions"));
    assert_eq!(button(&fixture, 1), listed);

    fixture.test.answer(
        request,
        Ok(vec![
            Revision {
                commit: [2; 32],
                time: 1_800_000_001_000,
                current: true,
            },
            Revision {
                commit: [1; 32],
                time: 1_800_000_000_000,
                current: false,
            },
        ]),
    );
    fixture.settle();
    assert_eq!(button(&fixture, 2), listed);
}
