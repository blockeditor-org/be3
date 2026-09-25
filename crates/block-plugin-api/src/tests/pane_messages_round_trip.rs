use super::*;

#[test]
fn pane_messages_round_trip() {
    let tree = PaneTree {
        items: vec![
            PaneItem::Split {
                horizontal: true,
                fraction: 0.25,
            },
            PaneItem::Tabs {
                count: 1,
                active: 0,
            },
            PaneItem::Pane(PaneId(1)),
            PaneItem::Tabs {
                count: 2,
                active: 1,
            },
            PaneItem::Pane(PaneId(2)),
            PaneItem::Group,
            PaneItem::Tabs {
                count: 1,
                active: 0,
            },
            PaneItem::Pane(PaneId(3)),
        ],
    };
    for message in [
        Message::Editor(EditorMessage::Panes {
            instance: EditorInstanceId(2),
            layout: PaneLayout {
                panes: vec![PaneInfo {
                    pane: PaneId(1),
                    title: "Files".into(),
                    closable: false,
                }],
                tree: tree.clone(),
                arrangement: 3,
            },
        }),
        Message::Editor(EditorMessage::ShowPane {
            instance: EditorInstanceId(2),
            pane: PaneId(3),
        }),
        Message::Editor(EditorMessage::PanesArranged {
            instance: EditorInstanceId(2),
            arrangement: 4,
            tree,
            detached: vec![PaneId(4)],
            focused: None,
        }),
        Message::Editor(EditorMessage::ClosePane {
            instance: EditorInstanceId(2),
            pane: PaneId(4),
        }),
        Message::Children(ChildPlacements {
            instance: EditorInstanceId(2),
            region: EditorRegion::Pane(PaneId(3)),
            generation: 1,
            children: Vec::new(),
            occluders: Vec::new(),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
