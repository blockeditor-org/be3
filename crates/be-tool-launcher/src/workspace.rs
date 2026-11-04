use beui::NodeId;
use beui::Overflow;
use beui::dock::Docking;
use beui::dock::{DockDocument, DockDocumentBody, DockDocumentTabs, DockEntry, DockPane, DockTab};
use beui::dock_host::styled::DockHost;
use beui::icons::ICON_TERMINAL;
use beui::reactive::*;
use beui::styled::*;

use crate::actions::describe;
use crate::detail::Detail;
use crate::github::PullRequest;
use crate::model::{Model, Tab};
use crate::pane::TerminalPane;
use crate::view::{PADDING, state_glyph};
use beui::{Align, Direction};

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
        <List spacing=Space::None>
            <Frame padding={Padding::symmetric(PADDING, Space::Lg)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{status}</Text>
                    </ListItem>
                    <Show condition={working}>
                        <Progress width=Size::Sm label="A build is running" />
                    </Show>
                </List>
            </Frame>
            <ListItem grow=1.0>
                <DockHost>
                    <Docking layout={model.layout}>
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
                </DockHost>
            </ListItem>
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
