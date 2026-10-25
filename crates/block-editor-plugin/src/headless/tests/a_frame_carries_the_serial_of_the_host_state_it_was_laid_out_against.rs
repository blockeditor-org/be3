use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, FrameCommit, HelloAccepted, Message, ScreenId,
    ScreenRequest, ScreenSet, Size, SurfaceFormat, SurfaceSpec, TimedInput, ViewportMetrics,
};
use uuid::Uuid;

use crate::headless::HeadlessPlugin;
use crate::{EditorHost, Frame, Instance, Region, Waker};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const SCREEN: ScreenId = ScreenId(1);

struct Still;

impl Instance for Still {
    fn connect(&mut self, _block_id: Uuid) {}

    fn input(&mut self, _region: &Region, _event: &TimedInput) {}

    fn update(&mut self, region: &Region, _settings: Option<&mut Vec<u8>>) -> Frame {
        Frame {
            painted: vec![region.rect],
            ..Frame::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &crate::PaintTarget<'_>) -> Vec<block_plugin_api::SurfaceRect> {
        target.whole()
    }
}

fn configure(plugin: &mut HeadlessPlugin, request_id: u64, height: f32, serial: u64) {
    plugin.receive(Message::Screens(ScreenSet {
        request_id,
        screens: vec![ScreenRequest {
            screen: SCREEN,
            instance: INSTANCE,
            region: EditorRegion::Frame,
            frame: None,
            metrics: ViewportMetrics {
                logical: Size {
                    width: 100.0,
                    height,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 100,
                pixel_height: height as u32,
                scale_factor: 1.0,
                monitors: Vec::new(),
            },
        }],
    }));
    plugin.receive(Message::Configure { serial });
}

fn drawn(plugin: &mut HeadlessPlugin) -> FrameCommit {
    plugin.draw();
    plugin
        .outbound()
        .into_iter()
        .find_map(|message| match message {
            Message::Frame(commit) => Some(commit),
            _ => None,
        })
        .expect("a drawn plugin commits a frame")
}

#[test]
fn a_frame_carries_the_serial_of_the_host_state_it_was_laid_out_against() {
    let mut plugin = HeadlessPlugin::new("still", "Still", "1");
    plugin.receive(Message::HelloAccepted(HelloAccepted {
        host_name: "test host".into(),
        surface: Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: true,
        }),
        preferences: block_plugin_api::Preferences::default(),
    }));
    plugin.adopt(INSTANCE, Box::new(Still), EditorHost::new(Waker::default()));
    plugin.receive(Message::Editor(EditorMessage::Open {
        instance: INSTANCE,
        editor: [6; 16],
        block_id: [1; 16],
        block_type: [2; 16],
        view_block: None,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    }));

    configure(&mut plugin, 1, 100.0, 3);
    let first = drawn(&mut plugin);
    assert_eq!(first.serial, 3);
    assert_eq!(first.reports.len(), 1, "the frame carries what it laid out");

    configure(&mut plugin, 2, 60.0, 4);
    let resized = drawn(&mut plugin);
    assert_eq!(
        resized.serial, 4,
        "the host stretches the painting and the children by the size it sent at this serial, \
         so a keyboard shrinking the editor reaches it even when no child moved"
    );
    assert_eq!(
        resized
            .sizes
            .iter()
            .map(|size| size.logical.height)
            .collect::<Vec<_>>(),
        [60.0]
    );
}
