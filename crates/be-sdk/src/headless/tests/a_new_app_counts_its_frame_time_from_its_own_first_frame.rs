use std::time::Instant;

use be_sdk_api::{HelloAccepted, SurfaceFormat, SurfaceSpec};

use crate::frame_time;
use crate::headless::HeadlessApp;

fn drawn(app: &mut HeadlessApp) {
    app.accept(HelloAccepted {
        host_name: "test host".into(),
        surface: Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: false,
        }),
        preferences: be_sdk_api::Preferences::default(),
    })
    .unwrap();
    app.draw_frame(0, 0, true).unwrap();
}

#[test]
fn a_new_app_counts_its_frame_time_from_its_own_first_frame() {
    let mut earlier = HeadlessApp::new("earlier", "Earlier", "0");
    drawn(&mut earlier);
    let started = Instant::now();
    let mut later = HeadlessApp::new("later", "Later", "0");
    drawn(&mut later);
    let now = frame_time().expect("a drawn frame sets the frame time");
    assert!(
        now >= started,
        "an app whose host clock starts again at zero starts its own frame clock, rather than reading its times against an earlier app's start: {now:?} is before {started:?}"
    );
}
