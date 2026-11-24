use super::*;
use be_sdk_api::{Input, InputBatch, ScreenId, TimedInput};

#[test]
fn an_app_not_running_yet_hears_only_where_the_keyboard_is() {
    let key = Input::Text("a".to_owned());
    let mut both = InputBatch::new(ScreenId(1), false);
    both.push(TimedInput::from(Input::Focus(true)));
    both.push(TimedInput::from(key.clone()));
    let mut keys = InputBatch::new(ScreenId(2), false);
    keys.push(TimedInput::from(key));
    let mut messages = vec![
        Outbound::App(AppCall::Input(both)),
        Outbound::App(AppCall::Input(keys)),
    ];
    keep_only_focus(&mut messages);
    let heard: Vec<(ScreenId, Vec<Input>)> = messages
        .iter()
        .map(|message| match message {
            Outbound::App(AppCall::Input(batch)) => (
                batch.screen,
                batch.events.iter().map(|event| event.input.clone()).collect(),
            ),
            _ => panic!("only input was sent"),
        })
        .collect();
    assert_eq!(
        heard,
        [(ScreenId(1), vec![Input::Focus(true)])],
        "the key is dropped, not queued, and a batch left empty is not sent"
    );
}
