use std::time::Instant;

use block_plugin_api::{HelloAccepted, SurfaceFormat, SurfaceSpec};

use crate::frame_time;
use crate::headless::HeadlessPlugin;

fn drawn(plugin: &mut HeadlessPlugin) {
    plugin
        .accept(HelloAccepted {
            host_name: "test host".into(),
            surface: Some(SurfaceSpec {
                format: SurfaceFormat::Rgba8Unorm,
                render: false,
            }),
            preferences: block_plugin_api::Preferences::default(),
        })
        .unwrap();
    plugin.draw_frame(0, 0, true).unwrap();
}

#[test]
fn a_new_plugin_counts_its_frame_time_from_its_own_first_frame() {
    let mut earlier = HeadlessPlugin::new("earlier", "Earlier", "0");
    drawn(&mut earlier);
    let started = Instant::now();
    let mut later = HeadlessPlugin::new("later", "Later", "0");
    drawn(&mut later);
    let now = frame_time().expect("a drawn frame sets the frame time");
    assert!(
        now >= started,
        "a plugin whose host clock starts again at zero starts its own frame clock, rather than reading its times against an earlier plugin's start: {now:?} is before {started:?}"
    );
}
