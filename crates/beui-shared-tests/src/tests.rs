use be_drive::Driven;
use beui_layer_drive::Reply;

const SIZE: &str = "1100x720";

fn run(fixture: &str, script: &str, delayed: bool) {
    let program = std::env::var("FIXTURES").expect(
        "FIXTURES names the fixtures program; run these through ./scripts/test //crates/beui-shared-tests:test",
    );
    let mode = if delayed {
        " with delayed measurement"
    } else {
        ""
    };
    let mut arguments = vec![fixture];
    if delayed {
        arguments.push("--delay-measurement");
    }
    let app = Driven::start(
        &program,
        &arguments,
        SIZE,
        std::env::var("APP_LIBRARIES").ok(),
        false,
    )
    .unwrap_or_else(|error| panic!("{fixture}{mode} did not start: {error}"));
    let commands = be_drive::commands(script)
        .unwrap_or_else(|error| panic!("{fixture}'s script does not parse: {error}"));
    for (line, words) in commands {
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        match app.request(&words) {
            Ok(Reply::Text(_)) => {}
            Ok(_) => panic!("{fixture}{mode}: `{line}` answered with something other than text"),
            Err(error) => panic!(
                "{fixture}{mode}: `{line}` failed: {error}\n\n{}",
                app.log_report()
            ),
        }
    }
    if let Err(error) = app.close() {
        panic!("{fixture}{mode} did not exit cleanly: {error}");
    }
}

fn drive(fixture: &str, script: &str) {
    run(fixture, script, false);
    run(fixture, script, true);
}

mod action_bar;
mod back_slide;
mod buttons;
mod canvas;
mod choices;
mod dates;
mod dialogs;
mod docking;
mod drag;
mod editing;
mod fields;
mod floating;
mod focus;
mod frame;
mod interactive;
mod list;
mod menus;
mod offset;
mod overlay;
mod palette;
mod pan_zoom;
mod picture;
mod pointer_lock;
mod popups;
mod portal;
mod progress;
mod rows;
mod scroll;
mod scroll_chain;
mod sheet;
mod stack;
mod stretch;
mod table;
mod text;
mod text_area;
mod text_input;
mod text_item;
mod time_list;
mod toasts;
mod toggles;
mod tree;
mod virtual_list;
