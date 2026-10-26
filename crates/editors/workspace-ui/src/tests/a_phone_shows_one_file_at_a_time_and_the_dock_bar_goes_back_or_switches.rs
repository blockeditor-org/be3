use block_editor_beui::{
    BarAction, ChildStatus, EditorCapabilities, EditorInstanceId, EditorRegion, InteractionMode,
    MenuCommand, MenuEntry, ResizeMode, TopBar,
};

use super::*;

fn placed(fixture: &Fixture, id: Uuid) -> Option<block_editor_beui::ChildPlacement> {
    fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content.block_id() == Some(id.into_bytes()))
        .cloned()
}

fn top_bar(fixture: &Fixture, id: Uuid) -> Option<TopBar> {
    placed(fixture, id).map(|placement| placement.top_bar)
}

fn bar(fixture: &mut Fixture, id: Uuid, action: BarAction) {
    let child = placed(fixture, id).expect("the block is on show").child;
    fixture.test.child_bar(child, action);
    fixture.settle();
}

fn placement_instance() -> EditorInstanceId {
    EditorInstanceId(0)
}

fn tap(fixture: &mut Fixture, test_id: &str) {
    fixture.test.click(test_id);
    fixture.settle();
}

#[test]
fn a_phone_shows_one_file_at_a_time_and_the_dock_bar_goes_back_or_switches() {
    let (mut fixture, first) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    let second = Uuid::new_v4();

    show(&mut fixture, first, None);
    assert_eq!(
        top_bar(&fixture, first),
        Some(TopBar::Phone),
        "an opened block fills the phone and leaves its bar to the dock"
    );

    show(&mut fixture, second, None);
    assert_eq!(fixture.shown(), vec![second], "one file is shown at a time");

    tap(&mut fixture, "dock.back");
    assert!(
        placed(&fixture, second).is_none(),
        "back closes the file opened from another"
    );
    assert_eq!(
        fixture.shown(),
        vec![first],
        "and shows the file it was opened from"
    );

    show(&mut fixture, second, None);
    tap(&mut fixture, "dock.switch");
    assert!(
        fixture.test.shown("dock.switcher.tab.1"),
        "the list of files is one of the open tabs"
    );
    fixture.test.settle();
    fixture
        .test
        .snapshot("a_phone_lists_its_open_files_to_switch_between");
    tap(&mut fixture, "dock.switcher.tab.2");
    assert_eq!(
        fixture.shown(),
        vec![first],
        "choosing a file shows that file"
    );
    assert_eq!(fixture.focused(), Some(first));

    tap(&mut fixture, "dock.switch");
    tap(&mut fixture, "dock.switcher.close.2");
    assert_eq!(
        fixture.shown(),
        vec![second],
        "closing the file on show moves to the one it was reached from"
    );
    assert_eq!(fixture.open_tabs(), 0, "a phone draws no tab bars");
    fixture.test.back();
    fixture.settle();
    fixture.test.settle();
    assert!(
        !fixture.test.shown("dock.switcher.tab.1"),
        "the back gesture closes the switcher"
    );
    assert_eq!(fixture.shown(), vec![second], "and leaves the file on show");

    tap(&mut fixture, "dock.menu");
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    assert!(
        text_within(&document, root, "Access").is_some()
            && text_within(&document, root, "Rename").is_none(),
        "before the block hands its menu over, the dock bar offers only the workspace's part of it"
    );
    drop(document);
    fixture.test.back();
    fixture.settle();
    let child = placed(&fixture, second)
        .expect("the block is on show")
        .child;
    fixture.test.report_children(|placement| ChildStatus {
        instance: placement_instance(),
        region: EditorRegion::Frame,
        child: placement.child,
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::None,
        error: None,
        menu: match placement.child == child {
            true => MenuEntry::flatten(&[MenuEntry::Command(MenuCommand {
                id: "editor.rename".into(),
                label: "Rename".into(),
                enabled: true,
                ..MenuCommand::default()
            })]),
            false => Vec::new(),
        },
        creation: None,
        preview: Default::default(),
        dialog: Default::default(),
    });
    fixture.settle();
    tap(&mut fixture, "dock.menu");
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    let rename = text_within(&document, root, "Rename").expect("the dock's menu lists Rename");
    let at = document
        .node_rect(rename)
        .expect("the row is laid out")
        .center();
    drop(document);
    fixture.test.click(at);
    fixture.settle();
    assert_eq!(
        fixture.test.take_child_menu_picks(),
        [(child, "editor.rename".to_owned())],
        "the dock bar's menu is the block's own, and picking from it reaches the block"
    );

    bar(&mut fixture, second, BarAction::Details);
    assert!(fixture.test.shown("workspace.details.rename"));
    fixture.test.settle();
    fixture
        .test
        .snapshot("a_phone_shows_a_files_details_in_a_sheet");
}
