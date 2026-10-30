use block_editor_beui::ShellCommand;
use block_editor_beui::be_block::ViewState;

use super::*;

fn truncated(saved: &EditorViewContent) -> ViewState {
    let mut layout = saved
        .root()
        .state("layout")
        .cloned()
        .expect("the layout is saved");
    layout.bytes.truncate(layout.bytes.len() / 2);
    layout
}

fn holding(layout: &ViewState) -> EditorViewContent {
    let mut saved = EditorView::document(None);
    let edit = saved
        .root()
        .set_state("layout", Some(layout), 1, Uuid::nil());
    saved.apply(&edit);
    saved
}

#[test]
fn a_desktop_whose_layout_cannot_be_read_keeps_it_until_reset() {
    let mut first = Fixture::new();
    first.settle();
    first.show_windows(&[host_window(1, true)]);
    assert_eq!(first.kept().len(), 1, "the window is kept");
    let unreadable = truncated(&first.saved());

    let mut second = Fixture::holding(holding(&unreadable));
    second.settle();
    second.show_windows(&[host_window(2, true)]);

    assert!(
        second.test.shown("workspace.layout.reset"),
        "the desktop asks before replacing the layout"
    );
    assert_eq!(
        second.saved().root().state("layout"),
        Some(&unreadable),
        "the saved layout is left as it was while the notice is open"
    );
    second
        .test
        .snapshot("a_desktop_whose_layout_cannot_be_read_keeps_it_until_reset");

    second.test.take_shell_commands();
    second.test.click("workspace.layout.exit");
    second.settle();
    let desktop = second.test.block_id().expect("the desktop is a block");
    assert_eq!(
        second.test.take_shell_commands(),
        vec![(desktop, ShellCommand::CloseEditor)],
        "exiting asks the host to end the desktop session"
    );
    assert_eq!(
        second.saved().root().state("layout"),
        Some(&unreadable),
        "exiting leaves the saved layout as it was"
    );

    second.test.click("workspace.layout.reset");
    second.settle();
    assert!(
        !second.test.shown("workspace.layout.reset"),
        "the notice closes"
    );
    let reset = second
        .saved()
        .root()
        .state("layout")
        .cloned()
        .expect("a layout is saved");
    assert_ne!(reset, unreadable, "the fresh layout is saved over it");
    let mut third = Fixture::holding(holding(&reset));
    third.settle();
    assert!(
        !third.test.shown("workspace.layout.reset"),
        "the fresh layout reads back"
    );
}
