use super::*;

mod a_failure_releases_what_the_remaps_held_down;
mod a_module_that_never_answers_runs_out_of_fuel_and_input_passes_through;
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
    let mut remapper = Remapper::new(MODULE).expect("the module loads");
    assert_eq!(remapper.configure(Some(rules.as_bytes().to_vec())), vec![]);
    remapper
}

fn hostile(handle: &str) -> Remapper {
    let module = format!(
        r#"
        (module
            (memory (export "memory") 1)
            (global $calls (mut i32) (i32.const 0))
            (data (i32.const 64) "\00\00\00\00\00\00\00\00\00\00")
            (data (i32.const 128) "\01\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\2d\00\00\00")
            (func (export "be_remap_buffer") (param i32) (result i32) (i32.const 1024))
            (func (export "be_remap_handle") (param i32) (result i64)
                (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
                {handle}))
        "#
    );
    let mut remapper = Remapper::new(module.as_bytes()).expect("the module is shaped like remaps");
    remapper.configure(Some(Vec::new()));
    remapper
}
