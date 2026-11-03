use beui::{WindowPlacement, WindowReport};

use super::*;

#[component]
fn Faded(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    view! {
        <Frame fill=ColorRole::Surface padding=Space::Xl3>
            <List spacing=Space::None>
                <Text style=TextStyle::HEADING>"Behind the window"</Text>
                <ForEach keys={ids}>|id: WindowId| {
                    let placement = WindowPlacement::Over {
                        window: beui::WindowId::MAIN,
                        at: pos2(40.0, 20.0),
                    };
                    view! {
                        <Window
                            id={None}
                            size=SHOWN
                            placement
                            on_report={|_: WindowReport| {}}
                        >|_: WindowHandle|
                            <WindowView windows id />
                        </Window>
                    }
                }</ForEach>
            </List>
        </Frame>
    }
}

#[test]
fn a_surfaces_alpha_multiplier_fades_it() {
    let mut harness = Harness::in_space(|windows| {
        view! {
            <Faded windows />
        }
    });
    let (window, id) = harness.open();
    let (width, height) = match harness.client.received.size {
        Some((width, height)) if width > 0 && height > 0 => (width, height),
        _ => (300, 200),
    };
    let modifier = harness.client.alpha_modifier(&window.surface);
    modifier.set_multiplier(u32::MAX / 2);
    let pixels = [0xb0, 0x6f, 0x2f, 0xff].repeat((width * height) as usize);
    harness
        .client
        .attach_pixels_unsent(&window, width, height, &pixels);
    harness.settle();

    let alpha = harness.compositor().server().state.layers(id)[0].alpha;
    assert!((alpha - 0.5).abs() < 0.001, "the surface is drawn at half alpha: {alpha}");
    let painting = harness.stepped.painting();
    beui_paint_snapshot::assert_painting("a_surfaces_alpha_multiplier_fades_it", painting);
}
