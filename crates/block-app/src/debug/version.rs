mod github;

#[cfg(target_os = "android")]
mod install;

use std::cell::RefCell;

use crate::platform::http;
use github::WorkflowRun;

use crate::ui::{RunView, VersionRuns, VersionView};

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    runs_fetch: Option<http::Fetch>,
    runs: Option<Result<Vec<WorkflowRun>, String>>,
    #[cfg(target_os = "android")]
    install: Option<install::Install>,
}

impl State {
    fn start_fetch(&mut self) {
        self.runs = None;
        self.runs_fetch = Some(http::Fetch::get(github::runs_url(), github::api_headers()));
    }
}

pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.runs.is_none() && state.runs_fetch.is_none() {
            state.start_fetch();
        }
    });
}

pub(crate) fn refresh() {
    STATE.with(|state| state.borrow_mut().start_fetch());
}

pub(crate) fn poll() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Some(fetch) = &state.runs_fetch {
            match fetch.poll() {
                Some(result) => {
                    state.runs = Some(result.and_then(|body| github::parse_runs(&body)));
                    state.runs_fetch = None;
                }
                None => crate::host::request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
        #[cfg(target_os = "android")]
        if let Some(install) = &mut state.install
            && !install.finished()
        {
            install.poll();
            crate::host::request_repaint_after(std::time::Duration::from_millis(100));
        }
    });
}

#[cfg(target_os = "android")]
pub(crate) fn install(run_id: u64) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let short = state
            .runs
            .as_ref()
            .and_then(|runs| runs.as_ref().ok())
            .and_then(|runs| runs.iter().find(|run| run.id == run_id))
            .map(|run| short_sha(&run.head_sha).to_owned());
        if let Some(short) = short {
            state.install = Some(install::Install::start(run_id, &short));
        }
    });
}

#[cfg(not(target_os = "android"))]
pub(crate) fn install(_run_id: u64) {}

pub(crate) fn view() -> VersionView {
    STATE.with(|state| {
        let state = state.borrow();
        VersionView {
            commit: crate::COMMIT.to_owned(),
            can_install: cfg!(target_os = "android"),
            runs: match &state.runs {
                None => VersionRuns::Loading,
                Some(Err(error)) => VersionRuns::Failed(error.clone()),
                Some(Ok(runs)) => {
                    VersionRuns::Loaded(runs.iter().map(|run| run_view(run, &state)).collect())
                }
            },
        }
    })
}

fn run_view(run: &WorkflowRun, state: &State) -> RunView {
    #[cfg(target_os = "android")]
    let (installing, error) = match &state.install {
        Some(install) if install.run_id == run.id => {
            (!install.finished(), install.error().map(str::to_owned))
        }
        _ => (false, None),
    };
    #[cfg(not(target_os = "android"))]
    let (installing, error) = {
        let _ = state;
        (false, None)
    };
    RunView {
        id: run.id,
        number: run.run_number,
        branch: run.head_branch.clone(),
        event: run.event.clone(),
        sha: short_sha(&run.head_sha).to_owned(),
        created: run.created_at.clone(),
        status: match &run.conclusion {
            Some(conclusion) => conclusion.clone(),
            None => run.status.clone(),
        },
        url: run.html_url.clone(),
        current: run.head_sha == crate::COMMIT,
        succeeded: run.conclusion.as_deref() == Some("success"),
        installing,
        error,
    }
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}
