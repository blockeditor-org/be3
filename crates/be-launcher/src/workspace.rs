use beui::NodeId;
use beui::icons::ICON_TERMINAL;
use beui::reactive::*;
use beui::styled::*;
use beui::dock::styled::Docking;
use beui::dock::unstyled::{DockDocument, DockDocumentBody, DockDocumentTabs, DockEntry, DockPane, DockTab};

use crate::actions::describe;
use crate::detail::Detail;
use crate::github::PullRequest;
use crate::model::{Model, Tab};
use crate::pane::TerminalPane;
use crate::view::{PADDING, state_glyph};

#[component]
pub(crate) fn Workspace(model: Model) -> NodeId {
    let runs = model.runs;
    let working = create_memo(move || runs.work.with(Option::is_some));
    let status = create_memo(move || match (runs.work.get(), runs.problem.get()) {
        (Some(work), _) => describe(&work),
        (None, Some((_, problem))) => problem,
        (None, None) => "Pick a pull request, or main, to build and run it.".to_owned(),
    });
    let pane = runs.pane;
    view! {
        <List spacing=0.0>
            <Frame padding={Sides::symmetric(PADDING, 12.0)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0) ellipsis=true>
                        {status}
                    </Text>
                    <Show condition={working}>
                        <Progress width=18.0 label="A build is running" />
                    </Show>
                </List>
            </Frame>
            <Docking @sizing=ItemSize::Percent(100.0) layout={model.layout}>
                <DockPane id="workspace" active={Some(Tab::Log)}>
                    <DockTab id=Tab::Log title="Log" icon=ICON_TERMINAL>
                        <Frame padding=PADDING>
                            <TerminalPane pane />
                        </Frame>
                    </DockTab>
                    <DockDocumentTabs
                        documents={model.documents}
                    >|document: DockDocument<Tab, PullRequest, u64>|
                        <PullRequestTab model document />
                    </DockDocumentTabs>
                </DockPane>
            </Docking>
        </List>
    }
}

#[component]
fn PullRequestTab(model: Model, document: DockDocument<Tab, PullRequest, u64>) -> DockEntry<Tab> {
    let pull_request = document.content();
    let title = create_memo(move || {
        pull_request.with(|pull_request| format!("#{} {}", pull_request.number, pull_request.title))
    });
    let icon = create_memo(move || {
        state_glyph(pull_request.with(|pull_request| pull_request.state)).to_owned()
    });
    view! {
        <DockTab id={document.key()} title icon on_close={move || document.close()}>
            <DockDocumentBody document>|pull_request: ReadSignal<PullRequest>|
                <Detail model pull_request={create_memo(move || pull_request.get())} />
            </DockDocumentBody>
        </DockTab>
    }
}
