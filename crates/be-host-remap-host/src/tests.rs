use super::*;

mod a_failure_releases_what_the_remaps_held_down;
mod a_key_held_before_the_remaps_saw_it_is_released_as_it_is;
mod a_modifier_a_session_chord_pressed_is_let_go_with_its_release;
mod a_module_that_never_answers_is_stopped_and_input_passes_through;
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

fn remapper(rules: &str) -> Remapper {
    let mut remapper = Remapper::new(compiled(MODULE));
    assert_eq!(remapper.configure(Some(rules.as_bytes().to_vec())), vec![]);
    remapper
}

/// Remaps written by hand: `handle` answers each request, counted in `$calls` (`Start` is the
/// first), with an `i64` holding where its reply is in memory and how long it is: an empty reply at
/// 64, ten bytes long, and one pressing X at 128, twenty.
fn compiled(module: &[u8]) -> Module {
    Remapper::compile(module.to_vec(), std::sync::Arc::new(|| {}))
        .wait(std::time::Duration::from_secs(60))
        .expect("the module compiles in time")
        .expect("the module is shaped like remaps")
}

fn hostile(handle: &str) -> Remapper {
    hostile_within(handle, Limits::default())
}

fn hostile_within(handle: &str, limits: Limits) -> Remapper {
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
    let mut remapper = Remapper::new(compiled(&module)).with_limits(limits);
    remapper.configure(Some(Vec::new()));
    remapper
}
