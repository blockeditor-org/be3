use beui::{WindowPlacement, WindowReport};

use super::*;

const PLACED: Pos2 = pos2(360.0, 160.0);

#[component]
fn Panels(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    view! {
        <Frame fill=ColorRole::Surface padding=Space::Xl3>
            <List spacing=Space::None>
                <Text style=TextStyle::HEADING>"The main panel"</Text>
                <ForEach keys={ids}>|id: WindowId| {
                    let placement = WindowPlacement::Over {
                        window: beui::WindowId::MAIN,
                        at: PLACED,
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

fn filled(width: i32, height: i32, [red, green, blue]: [u8; 3]) -> Vec<u8> {
    [blue, green, red, 0xff].repeat((width * height) as usize)
}

#[test]
fn a_toplevel_and_its_popup_are_panels_of_their_own_in_space() {
    let mut harness = Harness::in_space(|windows| {
        view! {
            <Panels windows />
        }
    });
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let size = harness.client.received.size.unwrap_or((300, 200));
    let (width, height) = match size {
        (width, height) if width > 0 && height > 0 => (width, height),
        _ => (300, 200),
    };
    let pixels = filled(width, height, [0x2f, 0x6f, 0xb0]);
    harness
        .client
        .attach_pixels_unsent(&window, width, height, &pixels);
    harness.settle();

    let menu = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (40, 60),
        (160, 120),
    );
    let pixels = filled(160, 120, [0xe0, 0x8a, 0x2c]);
    harness
        .client
        .attach_surface_pixels(&menu.surface, 160, 120, &pixels);
    harness.settle();

    let windows = harness
        .output
        .as_ref()
        .expect("a frame ran")
        .windows
        .iter()
        .map(|window| window.id)
        .collect::<Vec<_>>();
    assert_eq!(
        windows.len(),
        3,
        "the main panel, the toplevel's and its popup's are three windows: {windows:?}"
    );
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(id),
        "opening the popup's panel leaves the keyboard with its toplevel"
    );

    let painting = harness.stepped.painting();
    paint_snapshot::assert_painting(
        "a_toplevel_and_its_popup_are_panels_of_their_own_in_space",
        painting,
    );
}
