use super::*;
use block_plugin_api::{
    EditorRegion, HelloAccepted, InputBatch, OpenBlock, Opening, ScreenRequest, SurfaceFormat,
    SurfaceSpec, ViewportMetrics,
};

fn accepted(surface: Option<SurfaceSpec>) -> Delivery {
    Delivery::Accepted(HelloAccepted {
        host_name: "test host".into(),
        surface,
        preferences: block_plugin_api::Preferences::default(),
    })
}

fn accept(session: &mut ClientSession) {
    let surface = SurfaceSpec {
        format: SurfaceFormat::Rgba8Unorm,
        render: true,
    };
    session.receive(&accepted(Some(surface))).unwrap();
}

fn opening(instance: EditorInstanceId) -> Delivery {
    Delivery::Open(Opening::Block(OpenBlock {
        instance,
        editor: [6; 16],
        block_id: [1; 16],
        block_type: [2; 16],
        view_block: None,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    }))
}

fn input(screen: ScreenId) -> Delivery {
    Delivery::Input(InputBatch {
        screen,
        every_sample: false,
        events: Vec::new(),
    })
}

fn screen(screen: ScreenId, instance: EditorInstanceId) -> ScreenRequest {
    ScreenRequest {
        screen,
        instance,
        region: EditorRegion::Frame,
        frame: None,
        metrics: ViewportMetrics {
            logical: block_plugin_api::Size {
                width: 100.0,
                height: 100.0,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
            raster: 1.0,
            monitors: Vec::new(),
        },
    }
}

mod accepts_new_preferences_while_running;
mod accepts_ordered_lifecycle;
mod opens_and_closes_editor_instance;
mod refuses_to_draw_without_a_surface;
mod rejects_out_of_order_messages;
mod rejects_screens_for_unopened_instances;
