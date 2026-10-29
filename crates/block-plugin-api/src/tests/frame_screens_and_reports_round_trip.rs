use super::*;
use accesskit::{Node, NodeId, Role, Toggled, Tree, TreeId, TreeUpdate};

fn checklist_tree() -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_children(vec![NodeId(2)]);
    let mut done = Node::new(Role::CheckBox);
    done.set_label("Buy milk");
    done.set_value("x".repeat(MAX_STRING_BYTES + 1));
    done.set_toggled(Toggled::True);
    done.set_bounds(accesskit::Rect::new(8.0, 48.0, 208.0, 72.0));
    done.add_action(accesskit::Action::Click);
    done.add_action(accesskit::Action::Focus);
    done.set_author_id("checklist.done");
    TreeUpdate {
        nodes: vec![(NodeId(1), root), (NodeId(2), done)],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(2),
    }
}

#[test]
fn frame_screens_and_reports_round_trip() {
    let mut request = region_screen(EditorRegion::Frame, 1, 2, 640, 480, 1.0);
    request.frame = Some(FrameSpec {
        chrome: FrameChrome::Drawn,
        content: Some(Rect::from_min_size(
            Pos2::new(10.0, 20.0),
            Vec2::new(300.0, 200.0),
        )),
        top_bar: TopBar::Phone,
        dialog: Some(DialogFrame {
            narrow: true,
            bottom_inset: 24.0,
        }),
    });
    round_trips(&vec![request]);

    let frames = FrameCommit {
        reports: vec![FrameReport {
            screen: ScreenId(1),
            content: Rect::from_min_size(Pos2::new(0.0, 40.0), Vec2::new(640.0, 440.0)),
            painted: vec![Rect::from_min_size(
                Pos2::new(0.0, 0.0),
                Vec2::new(640.0, 40.0),
            )],
            floating: vec![Rect::from_min_size(
                Pos2::new(8.0, 44.0),
                Vec2::new(120.0, 90.0),
            )],
            floating_painted: vec![Rect::from_min_size(
                Pos2::new(8.0, 44.0),
                Vec2::new(120.0, 90.0),
            )],
            scrims: vec![Scrim {
                rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                color: crate::accessibility::Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 96,
                },
            }],
            drawing: vec![Rect::from_min_size(
                Pos2::new(0.0, 0.0),
                Vec2::new(640.0, 400.0),
            )],
            claims: vec![PressClaim {
                modifiers: Modifiers {
                    logo: true,
                    ..Modifiers::default()
                },
                rect: Rect::from_min_size(Pos2::new(0.0, 40.0), Vec2::new(320.0, 440.0)),
            }],
            description: Some(crate::accessibility::Description {
                tree: Some((&checklist_tree()).into()),
                actions: vec![DescribedAction {
                    id: "checklist.clear".to_owned(),
                    label: "Clear completed".to_owned(),
                    shortcut: Some("Ctrl+Shift+K".to_owned()),
                    enabled: false,
                    checked: None,
                    live: true,
                }],
            }),
        }],
        ..frame(1)
    };
    let encoded = crate::ops::encode(&frames);
    assert_eq!(
        crate::ops::encode(&crate::ops::decode::<FrameCommit>(&encoded).unwrap()),
        encoded,
        "AccessKit compares nodes by the order their properties were set in, so compare encodings"
    );
}
