use super::*;
use be_sdk_api::{
    HelloAccepted, InputBatch, OpenBlock, Opening, OpeningKind, ParentView, ScreenRequest, SurfaceFormat,
    SurfaceSpec, ViewId, ViewPlacement, ViewportMetrics,
};

fn accepted(surface: Option<SurfaceSpec>) -> Delivery {
    Delivery::Accepted(HelloAccepted {
        host_name: "test host".into(),
        surface,
        preferences: be_sdk_api::Preferences::default(),
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
    Delivery::Open(Opening {
        instance,
        editor: [6; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind: OpeningKind::Block(OpenBlock {
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: None,
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    })
}

fn input(screen: ScreenId) -> Delivery {
    Delivery::Input(InputBatch {
        screen,
        every_sample: false,
        events: Vec::new(),
        dismissed: Vec::new(),
    })
}

fn screen(screen: ScreenId, instance: EditorInstanceId) -> ScreenRequest {
    ScreenRequest {
        screen,
        instance,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: be_sdk_api::Size {
                width: 100.0,
                height: 100.0,
            },
            visible: be_sdk_api::Pos2::ZERO,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
            raster: 1.0,
        },
        windows: false,
        host_place: None,
        origin: None,
        docked: false,
        anchor_space: None,
        fit: false,
    }
}

mod accepts_new_preferences_while_running;
mod accepts_ordered_lifecycle;
mod opens_and_closes_editor_instance;
mod refuses_to_draw_without_a_surface;
mod rejects_out_of_order_messages;
mod rejects_screens_for_unopened_instances;
