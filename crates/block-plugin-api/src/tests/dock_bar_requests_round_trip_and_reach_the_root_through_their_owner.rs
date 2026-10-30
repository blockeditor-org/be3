use super::*;

#[test]
fn dock_bar_requests_round_trip_and_reach_the_root_through_their_owner() {
    let view = ViewRef {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
    };
    let requests = [
        DockRequest::PlaceBar(DockBarPlace {
            bar: DockTabPath::own(7),
            placement: Some(DockBarPlacement::Beside(DockBarEdge {
                edge: DockEdge::Right,
                align: DockAlign::Center,
            })),
        }),
        DockRequest::PlaceBar(DockBarPlace {
            bar: DockTabPath::own(7),
            placement: None,
        }),
        DockRequest::MoveBar(DockBarMove {
            bar: DockTabPath::own(7),
            at: Pos2::new(4.0, 6.0),
            stage: DockDragStage::Move,
        }),
        DockRequest::MoveBar(DockBarMove {
            bar: DockTabPath::own(7),
            at: Pos2::new(90.0, 6.0),
            stage: DockDragStage::Drop,
        }),
    ];
    let message = FrameCommit {
        docks: requests
            .iter()
            .map(|request| {
                DockMessage::Request(DockViewRequest {
                    view,
                    request: request.clone(),
                })
            })
            .collect(),
        ..frame(1)
    };
    round_trips(&message);
    assert_eq!(message.validate(), Ok(()));

    let child = ChildId(3);
    let through = requests[2].through(child, "host/03").unwrap();
    assert_eq!(
        through,
        DockRequest::MoveBar(DockBarMove {
            bar: DockTabPath::own(7).through(child),
            at: Pos2::new(4.0, 6.0),
            stage: DockDragStage::Move,
        })
    );
    let update = DockUpdate::ChildRequest(DockChildRequest {
        child,
        owner: Vec::new(),
        request: through,
    });
    round_trips(&update);
    assert_eq!(update.validate(), Ok(()));

    let lost = DockRequest::MoveBar(DockBarMove {
        bar: DockTabPath::own(7),
        at: Pos2::new(f32::NAN, 0.0),
        stage: DockDragStage::Cancel,
    });
    assert_eq!(lost.validate(), Err(DecodeError::MalformedPayload));
}
