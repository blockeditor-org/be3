use beui::styled::Root;
use std::time::Duration;

use super::*;

const ALARM: Duration = Duration::from_secs(60);

struct AlarmApp;

#[component]
impl BeuiApp for AlarmApp {
    #[component]
    fn View(_editor: Editor) -> Root {
        view! {
            <Root>
                <Alarm />
            </Root>
        }
    }
}

#[component]
fn Alarm() -> NodeId {
    let start = now();
    let rang = create_signal(None::<Duration>);
    let alarm = create_timer(move || {
        rang.set(Some(now() - start));
        None
    });
    alarm.start(ALARM);
    let text = create_memo(move || match rang.get() {
        Some(after) => after.as_millis().to_string(),
        None => "waiting".to_owned(),
    });
    view! {
        <Text @test_id={"alarm"}>{text}</Text>
    }
}

#[test]
fn a_plugin_timer_runs_on_the_tests_clock() {
    let mut test: BeuiTest<AlarmApp> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.run();
    assert_eq!(test.label("alarm"), "waiting");

    test.advance(ALARM - Duration::from_secs(1));
    test.run();
    assert_eq!(test.label("alarm"), "waiting");
    test.advance(Duration::from_secs(1));
    test.run();

    let rang: u128 = test.label("alarm").parse().expect("the alarm rang");
    let last_step = Duration::from_secs(1).as_millis();
    assert!(
        (ALARM.as_millis()..ALARM.as_millis() + last_step).contains(&rang),
        "the alarm rang {rang}ms after it was set"
    );
}
