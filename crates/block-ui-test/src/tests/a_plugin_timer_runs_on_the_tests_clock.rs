use std::time::Duration;

use beui::reactive::{create_memo, create_signal, create_timer, now};

use super::*;

const ALARM: Duration = Duration::from_secs(60);

struct AlarmApp;

impl BeuiApp for AlarmApp {
    fn view(_editor: Editor) -> NodeId {
        view! {
            <Alarm />
        }
    }
}

#[component]
fn Alarm() -> NodeId {
    let start = now();
    let (rang, set_rang) = create_signal(None::<Duration>);
    let alarm = create_timer(move || {
        set_rang.set(Some(now() - start));
        None
    });
    alarm.start(ALARM);
    let text = create_memo(move || match rang.get() {
        Some(after) => after.as_millis().to_string(),
        None => "waiting".to_owned(),
    });
    view! {
        <Text string={text} @test_id={"alarm"} />
    }
}

#[test]
fn a_plugin_timer_runs_on_the_tests_clock() {
    let mut test: BeuiTest<AlarmApp> =
        BeuiTest::new(Editor::new(EditorHost::default(), Uuid::new_v4()));
    test.run();
    assert_eq!(test.label("alarm"), "waiting");

    test.advance(ALARM - Duration::from_secs(1));
    assert_eq!(test.label("alarm"), "waiting");
    test.advance(Duration::from_secs(1));

    let rang: u128 = test.label("alarm").parse().expect("the alarm rang");
    assert!((ALARM.as_millis()..ALARM.as_millis() + 50).contains(&rang));
}
