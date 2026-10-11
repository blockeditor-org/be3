use std::time::Instant;

use block_plugin_api::Message;

use crate::frame_time;
use crate::headless::HeadlessPlugin;

#[test]
fn a_new_plugin_counts_its_frame_time_from_its_own_first_frame() {
    let mut earlier = HeadlessPlugin::new("earlier", "Earlier", "0");
    earlier.receive(Message::DrawFrame { now_micros: 0 });
    let started = Instant::now();
    let mut later = HeadlessPlugin::new("later", "Later", "0");
    later.receive(Message::DrawFrame { now_micros: 0 });
    let now = frame_time().expect("a drawn frame sets the frame time");
    assert!(
        now >= started,
        "a plugin whose host clock starts again at zero starts its own frame clock, rather than reading its times against an earlier plugin's start: {now:?} is before {started:?}"
    );
}
