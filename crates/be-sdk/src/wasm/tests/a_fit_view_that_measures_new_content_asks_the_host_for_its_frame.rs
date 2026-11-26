use be_sdk_app_host::{calls::Heard, wire::Wire};

use super::*;

#[test]
fn a_fit_view_that_measures_new_content_asks_the_host_for_its_frame() {
    // The frame that fitted a view presents nothing at the old size and owes the host the frame
    // that draws it at the new one. The runtime says so to the host on its own, with no input
    // and no wake to prompt it: a host that heard nothing would leave the view undrawn.
    let mut rig = Rig::new(Some(space()));
    rig.requests[0].fit = true;
    *rig.shared.fitted.borrow_mut() = Some(Vec2 { x: 50.0, y: 40.0 });
    let Rig {
        screens,
        surfaces,
        requests,
        ..
    } = rig;
    let wire = crate::wire::Shared::default();
    let executor = wire.executor();
    let mut runtime = crate::runtime::Runtime::from_parts(
        screens,
        surfaces,
        crate::Waker::default(),
        &[
            accepted(SurfaceSpec {
                format: SurfaceFormat::Rgba8Unorm,
                render: true,
            }),
            opening(),
        ],
    );
    let batch = vec![
        Delivery::Screens(requests),
        Delivery::Configure(1),
        Delivery::Draw {
            now_micros: 1,
            wall_micros: 1,
            seen: true,
        },
    ];
    executor
        .enter(|| runtime.step(batch, false, &wire))
        .expect("the frame runs");
    let mut sent = Vec::new();
    executor
        .step(Vec::new(), &mut sent)
        .expect("the host's frames decode");
    let mut host = Wire::default();
    let mut heard = Vec::new();
    for frame in sent {
        host.receive(&frame, &mut heard)
            .expect("the app's frames decode");
    }
    assert!(
        heard.iter().any(|heard| matches!(heard, Heard::FrameNeeded)),
        "the app asks for the frame that draws the fitted view, with nothing else to prompt it"
    );
}
