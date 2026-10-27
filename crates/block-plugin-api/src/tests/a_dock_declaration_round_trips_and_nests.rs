use super::*;

#[test]
fn a_dock_declaration_round_trips_and_nests() {
    let declaration = declaration();
    let message = FrameCommit {
        docks: vec![DockMessage::Declare(DockDeclare {
            region: RegionRef {
                instance: EditorInstanceId(1),
                region: EditorRegion::Frame,
            },
            declaration: Some(declaration.clone()),
        })],
        ..frame(1)
    };
    round_trips(&message);
    assert_eq!(message.validate(), Ok(()));
    let nodes = declaration.nest().unwrap();
    assert_eq!(nodes.tabs().len(), 3);
    assert_eq!(
        DockTree::flatten(&nodes, declaration.tree.focus.clone()),
        declaration.tree
    );
    assert_eq!(
        DockTabPath::own(4).through(ChildId(9)).within(ChildId(9)),
        Some(DockTabPath::own(4))
    );

    let mut undescribed = declaration.clone();
    undescribed.tabs.pop();
    assert_eq!(undescribed.validate(), Err(DecodeError::MalformedPayload));

    let mut lopsided = declaration.clone();
    lopsided.tree.items.remove(1);
    lopsided.tree.items.remove(1);
    lopsided.tabs.remove(0);
    assert_eq!(lopsided.validate(), Err(DecodeError::MalformedPayload));

    let mut deep = declaration;
    deep.tree.items.last_mut().unwrap().kind = DockItemKind::Tab(DockTabPath {
        via: vec![ChildId(1); MAX_DOCK_NESTING + 1],
        tab: 4,
    });
    assert_eq!(
        deep.validate(),
        Err(DecodeError::LimitExceeded("dock nesting"))
    );
}
