use std::sync::mpsc::channel;

use beui::dock::styled::TabId;
use beui::reactive::untrack;
use beui::{Input, Modifiers, Vec2};
use block_ui_test::DocumentTest;

use super::{Person, PullRequest, State, scratch};
use crate::LauncherApp;
use crate::github::Filter;
use crate::model::{Model, Tab};
use crate::pane::{Pane, Session};
use crate::runs::Runs;
use crate::tasks::{self, Tasks};

const WIDE: Vec2 = Vec2::new(1200.0, 800.0);
const LATER: i64 = i64::MAX / 2;

fn pull_request(number: u64, title: &str) -> PullRequest {
    PullRequest {
        number,
        title: title.to_owned(),
        state: State::Open,
        author: Person {
            login: "someone".to_owned(),
            avatar: String::new(),
        },
        branch: format!("branch-{number}"),
        head_sha: format!("{number:040}"),
        same_repository: true,
        labels: Vec::new(),
        created: LATER,
        updated: LATER,
        url: String::new(),
        body: String::new(),
    }
}

fn tab(model: Model, number: u64) -> Option<TabId> {
    let slot = untrack(|| model.documents.find(&number))?;
    model.layout.tab_of(&Tab::PullRequest(slot))
}

fn close(model: Model, number: u64) -> String {
    let tab = tab(model, number).expect("the pull request has a tab");
    format!("dock.tab.{}.close", tab.value())
}

fn click(test: &mut DocumentTest, number: u64, modifiers: Modifiers) {
    test.frame(vec![Input::Modifiers(modifiers)]);
    test.click(format!("launcher.pull_request.{number}"));
    test.frame(vec![Input::Modifiers(Modifiers::NONE)]);
}

#[test]
fn pull_requests_open_in_place_of_the_one_shown_unless_ctrl_is_held() {
    let (sender, events) = channel();
    let tasks = Tasks::new(scratch("tabs"), sender);
    let session = Session::new().expect("a terminal");
    let LauncherApp {
        document, model, ..
    } = LauncherApp::with(tasks, events, move |tasks| {
        let runs = Runs::new(tasks.clone(), Pane::new(session));
        Model::new(tasks, runs)
    });
    model.receive(tasks::Event::PullRequests(
        Filter::Open,
        Ok(vec![
            pull_request(1, "Make the launcher tabbed"),
            pull_request(2, "Fix the build"),
            pull_request(3, "Speed up the tests"),
        ]),
    ));
    let mut test = DocumentTest::new(document, WIDE);
    let is_open = |test: &DocumentTest, number: u64| {
        tab(model, number).is_some_and(|tab| test.shows(&format!("dock.tab.{}.close", tab.value())))
    };

    click(&mut test, 1, Modifiers::NONE);
    assert!(is_open(&test, 1), "a click opens the pull request in a tab");

    click(&mut test, 2, Modifiers::NONE);
    assert!(
        is_open(&test, 2) && !is_open(&test, 1),
        "the next click shows its pull request in the same tab"
    );

    click(&mut test, 3, Modifiers::CTRL);
    assert!(
        is_open(&test, 2) && is_open(&test, 3),
        "Ctrl and a click opens a tab of its own"
    );

    click(&mut test, 2, Modifiers::NONE);
    assert!(
        is_open(&test, 2) && is_open(&test, 3),
        "a click on a pull request that is open shows its tab"
    );

    click(&mut test, 1, Modifiers::NONE);
    assert!(
        is_open(&test, 1) && is_open(&test, 3) && !is_open(&test, 2),
        "and the next click replaces the one shown last"
    );
    assert!(
        test.rect_of(&close(model, 1)).min.x < test.rect_of(&close(model, 3)).min.x,
        "in the tab it had"
    );

    test.click(close(model, 3));
    test.frame(Vec::new());
    assert!(
        !is_open(&test, 3) && is_open(&test, 1),
        "a pull request's tab closes from its close button"
    );
}
