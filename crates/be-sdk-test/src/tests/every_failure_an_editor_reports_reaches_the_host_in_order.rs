use beui::styled::Root;

use super::*;

struct FailingRoot;

#[component]
impl BeuiRoot for FailingRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let host = editor.host();
        host.report_failure("The first thing failed.");
        host.report_failure("The second thing failed.");
        view! {
            <Root>
                <Text>"Failing"</Text>
            </Root>
        }
    }
}

#[test]
fn every_failure_an_editor_reports_reaches_the_host_in_order() {
    let mut test = BeuiTest::<FailingRoot>::editing(Uuid::new_v4());
    test.run();

    assert_eq!(
        test.take_reported_failures(),
        ["The first thing failed.", "The second thing failed."],
        "each failure is told, none replaces another"
    );
    assert!(
        test.take_reported_failures().is_empty(),
        "a failure is taken once"
    );
}

test_editor!(FailingRoot);
