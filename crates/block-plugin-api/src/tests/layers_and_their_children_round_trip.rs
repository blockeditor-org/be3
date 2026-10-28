use super::*;

#[test]
fn layers_and_their_children_round_trip() {
    let mut request = screen(1, 2, 640, 480, 2.0);
    request.layers = Some(LayerSpace {
        to_host: HostTransform {
            origin: Pos2::new(200.0, 80.0),
            scale: Vec2::new(1.0, 1.0),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
    });
    round_trips(&vec![request]);

    let menu = Rect::from_min_size(Pos2::new(210.0, 120.0), Vec2::new(180.0, 240.0));
    let submenu = Rect::from_min_size(Pos2::new(390.0, 160.0), Vec2::new(160.0, 120.0));
    let sheet = Rect::from_min_size(Pos2::new(0.0, 500.0), Vec2::new(1280.0, 300.0));
    let mut popover = layer(
        3,
        LayerParent::Region,
        LayerPlacement::Edge(LayerEdge::Bottom),
        sheet,
    );
    popover.scrims = vec![Scrim {
        rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
        color: crate::accessibility::Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 96,
        },
    }];
    popover.blocks_owner = true;
    let frame = FrameCommit {
        reports: vec![report(
            1,
            vec![
                layer(
                    1,
                    LayerParent::Region,
                    LayerPlacement::Anchored(Rect::from_min_size(
                        Pos2::new(10.0, 10.0),
                        Vec2::new(24.0, 24.0),
                    )),
                    menu,
                ),
                layer(
                    2,
                    LayerParent::Layer(LayerId(1)),
                    LayerPlacement::Anchored(Rect::from_min_size(
                        Pos2::new(210.0, 160.0),
                        Vec2::new(180.0, 28.0),
                    )),
                    submenu,
                ),
                popover,
            ],
        )],
        children: vec![ChildPlacements {
            instance: EditorInstanceId(2),
            region: EditorRegion::Frame,
            children: vec![ChildPlacement {
                child: ChildId(4),
                content: ChildContent::Picker {
                    filter: BlockFilter::default(),
                    parent: BlockLocation::Root,
                    touch: true,
                },
                rect: sheet,
                clip: sheet,
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 12.0,
                layer: ChildLayer::Above,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
                dialog: None,
                docks: None,
                in_layer: Some(LayerId(3)),
            }],
            occluders: Vec::new(),
        }],
        ..super::frame(1)
    };
    assert_eq!(frame.validate(), Ok(()));
    round_trips(&frame);

    round_trips(&InputBatch {
        screen: ScreenId(1),
        events: vec![TimedInput {
            layer: Some(LayerId(2)),
            ..Input::PointerMoved(Pos2::new(200.0, 90.0)).into()
        }],
        every_sample: false,
        dismissed: vec![LayerId(3)],
    });
}
