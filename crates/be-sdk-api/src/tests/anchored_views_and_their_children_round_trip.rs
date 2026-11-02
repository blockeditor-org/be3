use super::*;

#[test]
fn anchored_views_and_their_children_round_trip() {
    let mut request = screen(1, 2, 640, 480, 2.0);
    request.anchor_space = Some(AnchorSpace {
        to_host: HostTransform {
            origin: Pos2::new(200.0, 80.0),
            scale: Vec2::new(1.0, 1.0),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
    });
    round_trips(&vec![request]);

    let (menu, submenu, sheet) = (
        ScreenId::opened(1),
        ScreenId::opened(2),
        ScreenId::opened(3),
    );
    round_trips(&ScreenLayout {
        screens: [(menu, 11), (submenu, 12), (sheet, 13)]
            .into_iter()
            .map(|(screen, view)| ScreenPlacement {
                screen,
                instance: EditorInstanceId(2),
                view: ViewId(view),
                surface: 7,
                width: 360,
                height: 480,
                scale_factor_millis: 2000,
                raster_millis: 2000,
            })
            .collect(),
    });
    let sheet_rect = Rect::from_min_size(Pos2::new(0.0, 500.0), Vec2::new(1280.0, 300.0));
    let mut sheet_report = anchored_report(
        sheet,
        AnchoredPlacement {
            blocks_owner: true,
            ..anchored(0, ViewAnchor::Edge(ViewEdge::Bottom), sheet_rect)
        },
    );
    sheet_report.scrims = vec![Scrim {
        rect: Rect::from_min_size(Pos2::new(0.0, -500.0), Vec2::new(1280.0, 800.0)),
        color: crate::accessibility::Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 96,
        },
        radius: 10.0,
    }];
    let frame = FrameCommit {
        reports: vec![
            anchored_report(
                menu,
                anchored(
                    0,
                    ViewAnchor::Rect(Rect::from_min_size(
                        Pos2::new(10.0, 10.0),
                        Vec2::new(24.0, 24.0),
                    )),
                    Rect::from_min_size(Pos2::new(210.0, 120.0), Vec2::new(180.0, 240.0)),
                ),
            ),
            anchored_report(
                submenu,
                anchored(
                    11,
                    ViewAnchor::Rect(Rect::from_min_size(
                        Pos2::new(210.0, 160.0),
                        Vec2::new(180.0, 28.0),
                    )),
                    Rect::from_min_size(Pos2::new(390.0, 160.0), Vec2::new(160.0, 120.0)),
                ),
            ),
            sheet_report,
        ],
        children: vec![ChildPlacements {
            instance: EditorInstanceId(2),
            view: ViewId(13),
            children: vec![ChildPlacement {
                child: ChildId(4),
                content: ChildContent::Picker {
                    filter: BlockFilter::default(),
                    parent: BlockLocation::Root,
                },
                rect: Rect::from_min_size(Pos2::ZERO, sheet_rect.size()),
                clip: Rect::from_min_size(Pos2::ZERO, sheet_rect.size()),
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
                anchor: None,
            }],
            occluders: Vec::new(),
        }],
        ..super::frame(1)
    };
    assert_eq!(frame.validate(), Ok(()));
    round_trips(&frame);

    round_trips(&InputBatch {
        screen: submenu,
        events: vec![Input::PointerMoved(Pos2::new(20.0, 10.0)).into()],
        every_sample: false,
        dismissed: Vec::new(),
    });
    round_trips(&InputBatch {
        screen: ScreenId(1),
        events: Vec::new(),
        every_sample: false,
        dismissed: vec![ViewId(13)],
    });
}
