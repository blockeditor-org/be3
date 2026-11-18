use be_sdk_app_host::pacing::{Ask, Asking};

use super::*;

mod an_app_owes_its_root_only_what_is_due_now;

const NOW: Duration = Duration::from_secs(5);

/// Asks `pacing` for a frame at `NOW`, as `State::ask` does for an app with a surface.
fn asked(pacing: &mut Pacing) {
    let ask = pacing.ask(Asking {
        drawing: true,
        seen: true,
        now: NOW,
        pass: 1,
        held: false,
        holds_input: false,
    });
    assert_eq!(ask, Ask::Draw, "a frame needed is asked for");
}
