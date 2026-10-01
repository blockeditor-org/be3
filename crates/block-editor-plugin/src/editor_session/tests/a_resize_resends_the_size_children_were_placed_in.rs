use super::*;

struct Still;

impl Instance for Still {
    fn connect(&mut self, _block_id: Uuid) {}

    fn input(&mut self, _region: &Region, _event: &InputEvent) {}

    fn update(&mut self, region: &Region, _settings: Option<&mut Vec<u8>>) -> Frame {
        Frame {
            painted: vec![region.rect],
            ..Frame::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<block_plugin_api::SurfaceRect> {
        target.whole()
    }
}

fn place(session: &mut EditorSession, height: u32) {
    let screen = block_plugin_api::ScreenId(1);
    let instance = EditorInstanceId(1);
    session.place(
        &[ScreenPlacement {
            screen,
            instance,
            region: EditorRegion::Frame,
            x: 0,
            y: 0,
            width: 100,
            height,
            scale_factor_millis: 1000,
        }],
        &[ScreenRequest {
            screen,
            instance,
            region: EditorRegion::Frame,
            frame: None,
            metrics: ViewportMetrics {
                logical_width: 100.0,
                logical_height: height as f32,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 100,
                pixel_height: height,
                scale_factor: 1.0,
            },
        }],
    );
    session.run(EditorRegion::Frame, 1);
}

fn sizes(session: &mut EditorSession) -> Vec<Size> {
    session
        .outbound()
        .into_iter()
        .filter_map(|message| match message {
            Message::Children(placements) => Some(placements.size),
            _ => None,
        })
        .collect()
}

#[test]
fn a_resize_resends_the_size_children_were_placed_in() {
    let mut session = EditorSession::adopt(
        EditorInstanceId(1),
        Box::new(Still),
        EditorHost::new(Waker::default()),
    );

    place(&mut session, 100);
    assert_eq!(
        sizes(&mut session),
        [Size {
            width: 100.0,
            height: 100.0
        }]
    );

    place(&mut session, 60);
    assert_eq!(
        sizes(&mut session),
        [Size {
            width: 100.0,
            height: 60.0
        }],
        "the host stretches the painting and the children by this size, so a keyboard \
         shrinking the editor must reach it even when no child moved"
    );
}
