use be_drive::Driven;
use beui_layer_drive::Reply;

const SIZE: &str = "1100x720";

fn words(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut characters = line.chars().peekable();
    while let Some(&next) = characters.peek() {
        if next.is_whitespace() {
            characters.next();
            continue;
        }
        let mut word = String::new();
        match next {
            '\'' => {
                characters.next();
                word.extend(
                    characters
                        .by_ref()
                        .take_while(|character| *character != '\''),
                );
            }
            '"' => {
                characters.next();
                while let Some(character) = characters.next() {
                    match character {
                        '"' => break,
                        '\\' => word.extend(characters.next()),
                        _ => word.push(character),
                    }
                }
            }
            _ => {
                while let Some(&character) = characters.peek() {
                    if character.is_whitespace() {
                        break;
                    }
                    word.push(character);
                    characters.next();
                }
            }
        }
        found.push(word);
    }
    found
}

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
    for line in script.lines() {
        let words = words(line);
        if words.first().is_none_or(|first| first.starts_with('#')) {
            continue;
        }
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

mod canvas;
mod focus;
mod frame;
mod interactive;
mod list;
mod offset;
mod overlay;
mod portal;
mod text;
mod virtual_list;
