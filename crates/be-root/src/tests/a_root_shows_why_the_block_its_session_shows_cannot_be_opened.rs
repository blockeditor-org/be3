use crate::{SESSION_NOTICE_LAYER, SESSION_NOTICE_SURFACE};
use be_wasm_root_api::{CoreView, EditorApp};

use super::*;

/// Draws frames from `*time` on until the root asks its host for the editor of a block, and
/// answers which block.
fn asked_editor(root: &mut Root, time: &mut u64) -> (u64, [u8; 16]) {
    (0..12)
        .find_map(|_| {
            *time += 1;
            step(root, *time);
            root.take_editor_asks()
                .into_iter()
                .next()
                .map(|(id, shown)| (id, shown.block))
        })
        .expect("within a few frames, the root asks which app edits the block its session shows")
}

#[test]
fn a_root_shows_why_the_block_its_session_shows_cannot_be_opened() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // The organization's blocks are open and its session shows a block: the root asks which
    // app edits it, showing that its app loads meanwhile.
    show_core(
        &mut root,
        CoreView {
            showing: Some([7; 16]),
            ..CoreView::default()
        },
    );
    let (asked, block) = asked_editor(&mut root, &mut time);
    assert_eq!(
        block, [7; 16],
        "the root asks of the block its session shows"
    );
    let loading = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the session's notice shows its app loading",
        |done| layer_of(done, SESSION_NOTICE_LAYER).is_some(),
    );
    assert!(layer_of(&loading, SESSION_NOTICE_LAYER).is_some());

    // The host names no app for it: the notice says so, in the host's words, with no Reload.
    root.answer_editor(asked, Err("no app edits the block's type".to_owned()));
    let painting = painting_of(&mut root, &mut time, SESSION_NOTICE_SURFACE);
    beui_paint_snapshot::assert_painting(
        "a_root_shows_why_the_block_its_session_shows_cannot_be_opened",
        painting,
    );

    // Another block, whose app the host names but will not open in an instance that does not
    // run it: the root spawned it, and asks the host to open the block's editor in it.
    show_core(
        &mut root,
        CoreView {
            showing: Some([8; 16]),
            ..CoreView::default()
        },
    );
    let (asked, block) = asked_editor(&mut root, &mut time);
    assert_eq!(block, [8; 16]);
    root.answer_editor(
        asked,
        Ok(EditorApp {
            app: "be3.checklist".to_owned(),
            module: "checklist.wasm".to_owned(),
            name: "Checklist".to_owned(),
        }),
    );
    let opening = (0..12)
        .find_map(|_| {
            time += 1;
            step(&mut root, time);
            root.take_openings().into_iter().next()
        })
        .expect("within a few frames, the root asks its host to open the block's editor");
    assert_eq!(opening.shown.block, [8; 16]);
    assert_eq!(
        opening.app, None,
        "the app could not start here, where nothing is staged, so its instance runs none"
    );
    root.answer_opening(
        opening.id,
        Err("be3.checklist edits the block, which the root's instance does not run".to_owned()),
    );
    let shown = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the session's notice still shows",
        |done| layer_of(done, SESSION_NOTICE_LAYER).is_some(),
    );
    assert!(layer_of(&shown, SESSION_NOTICE_LAYER).is_some());
}
