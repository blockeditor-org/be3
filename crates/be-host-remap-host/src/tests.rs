use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, mpsc},
    time::Duration,
};

use super::*;

mod a_failure_releases_what_the_remaps_held_down;
mod a_key_held_before_the_remaps_saw_it_is_released_as_it_is;
mod a_modifier_a_session_chord_pressed_is_let_go_with_its_release;
mod a_module_that_never_answers_is_stopped_and_input_passes_through;
mod a_module_that_never_starts_holds_up_no_input;
mod new_rules_apply_from_the_next_key;
mod remaps_that_do_not_read_stay_off_until_they_change;
mod remaps_that_keep_failing_stop_until_they_change;
mod the_module_remaps_input;
mod the_session_chords_and_the_lock_screen_are_never_remapped;

const MODULE: &[u8] = include_bytes!(env!("REMAP_WASM"));
const NOW: u64 = 1_000_000;

fn key(name: &str) -> Source {
    Source::named(name).expect("a known name")
}

fn press(name: &str) -> Input {
    Input::Press(key(name))
}

fn release(name: &str) -> Input {
    Input::Release(key(name))
}

/// Remaps and what their instance's wake signals.
struct Remaps {
    remapper: Remapper,
    woken: mpsc::Receiver<()>,
}

impl Deref for Remaps {
    type Target = Remapper;

    fn deref(&self) -> &Remapper {
        &self.remapper
    }
}

impl DerefMut for Remaps {
    fn deref_mut(&mut self) -> &mut Remapper {
        &mut self.remapper
    }
}

impl Remaps {
    fn new(module: &[u8], limits: Limits) -> Self {
        let (wake, woken) = mpsc::channel();
        let module = Remapper::compile(module.to_vec(), Arc::new(|| {}))
            .wait(Duration::from_secs(60))
            .expect("the module compiles in time")
            .expect("the module is shaped like remaps");
        let wake: Wake = Arc::new(move || {
            let _ = wake.send(());
        });
        Self {
            remapper: Remapper::new(module, wake).with_limits(limits),
            woken,
        }
    }

    /// Polls until the remaps answered `Start`, failed for good, or should not run.
    fn started(&mut self) -> Vec<Output> {
        let mut out = Vec::new();
        loop {
            out.extend(self.remapper.poll(NOW));
            if !self.remapper.starting() {
                return out;
            }
            // Failed, to be started again by the next poll.
            if self.remapper.running.is_none() {
                continue;
            }
            self.woken
                .recv_timeout(Duration::from_secs(60))
                .expect("the remaps answer their start");
        }
    }
}

fn remapper(rules: &str) -> Remaps {
    let mut remaps = Remaps::new(MODULE, Limits::default());
    assert_eq!(remaps.configure(Some(rules.as_bytes().to_vec())), vec![]);
    assert_eq!(remaps.started(), vec![]);
    remaps
}

fn hostile(handle: &str) -> Remaps {
    hostile_within(handle, Limits::default())
}

/// Remaps written by hand: `handle` answers each request, counted in `$calls` (`Start` is the
/// first), with an `i64` holding where its reply is in memory and how long it is: an empty reply at
/// 64, ten bytes long, and one pressing X at 128, twenty. They are not started yet.
fn hostile_within(handle: &str, limits: Limits) -> Remaps {
    let module = format!(
        r#"
        (module
            (import "be3_host" "host_send" (func $send (param i32 i32)))
            (import "be3_host" "host_receive" (func $receive (param i32 i32) (result i64)))
            (memory (export "memory") 1)
            (global $calls (mut i32) (i32.const 0))
            (data (i32.const 64) "\00\00\00\00\00\00\00\00\00\00")
            (data (i32.const 128) "\01\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\2d\00\00\00")
            (func (export "app_start"))
            (func (export "app_shutdown"))
            (func (export "app_step") (local $answer i64)
                (block $read
                    (loop $requests
                        (br_if $read
                            (i64.eq
                                (call $receive (i32.const 1024) (i32.const 4096))
                                (i64.const -1)))
                        (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
                        (local.set $answer {handle})
                        (call $send
                            (i32.wrap_i64 (i64.shr_u (local.get $answer) (i64.const 32)))
                            (i32.wrap_i64 (local.get $answer)))
                        (br $requests)))))
        "#
    );
    let module = wat::parse_str(&module).expect("the module is written well");
    let mut remaps = Remaps::new(&module, limits);
    remaps.configure(Some(Vec::new()));
    remaps
}
