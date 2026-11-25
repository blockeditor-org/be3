use be_lib_layers::{Affine, Area, LayerInput, Surface};
use be_sdk_api::HostWindowId;

use super::*;

#[test]
fn a_program_window_is_a_host_layer_at_its_place_as_big_as_its_place() {
    // A window placed at (10, 20) in editor 1's view, 320 by 240 points, of which its placer's
    // clip shows up to (200, 200).
    let mut placement = placed(rect((10.0, 20.0), (330.0, 260.0)), ChildMode::Active);
    placement.content = ChildContent::Window(HostWindowId(9));
    placement.clip = rect((0.0, 0.0), (200.0, 200.0));
    let mut children = placing(Vec::new());
    children.windows.push((
        WindowPlace {
            editor: EditorInstanceId(1),
            view: ViewId::MAIN,
            window: HostWindowId(9),
            size: be_lib_geometry::vec2(320.0, 240.0),
        },
        placement,
    ));
    // The placer's view, 400 by 300 points laid out at 2 pixels a point, drawn at (50, 0) in
    // the window.
    let placer = Layer {
        id: PLACER,
        surface: Surface::Screen {
            instance: 5,
            surface: 0,
        },
        size: (800, 600),
        transform: Affine {
            xx: 0.5,
            xy: 0.0,
            yx: 0.0,
            yy: 0.5,
            x: 50.0,
            y: 0.0,
        },
        clip: rect((0.0, 0.0), (800.0, 600.0)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    };
    let mut layers = Vec::new();
    children.layers(&screen(), placer, &mut layers);
    let at = |surface: Surface| {
        layers
            .iter()
            .position(|layer| layer.surface == surface)
            .expect("the layer shows")
    };
    let window = &layers[at(Surface::Host(9))];
    // Its pixels are the window's logical pixels, as many as its place has points: what the
    // host configures it to.
    assert_eq!(window.size, (320, 240));
    assert_eq!(window.transform.apply(pos2(0.0, 0.0)), pos2(60.0, 20.0));
    assert_eq!(window.transform.apply(pos2(320.0, 240.0)), pos2(380.0, 260.0));
    // It shows only what its place's clip lets through.
    assert_eq!(window.clip, rect((0.0, 0.0), (190.0, 180.0)));
    // It takes the presses where it is, below its placer, which is hit around it.
    assert_eq!(window.input.hit, Area::Whole);
    assert_eq!(window.input.placer, Some(PLACER));
    assert!(at(Surface::Host(9)) < at(placer_surface()));
    assert_ne!(layers[at(placer_surface())].input.hit, Area::Whole);
}

fn placer_surface() -> Surface {
    Surface::Screen {
        instance: 5,
        surface: 0,
    }
}

/// Editor 1's main view, 400 by 300 points at 2 pixels a point.
fn screen() -> ScreenRequest {
    ScreenRequest {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        placement: be_sdk_api::ViewPlacement::Parent(be_sdk_api::ParentView::Main),
        frame: None,
        metrics: be_sdk_api::ViewportMetrics {
            logical: be_lib_geometry::vec2(400.0, 300.0),
            visible: pos2(0.0, 0.0),
            pixel_width: 800,
            pixel_height: 600,
            scale_factor: 2.0,
            raster: 2.0,
        },
        windows: false,
        host_place: None,
        origin: None,
        docked: false,
        anchor_space: None,
        fit: false,
    }
}
