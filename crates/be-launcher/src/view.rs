use beui::reactive::{
    ForEach, KeyedStore, List, ReadSignal, Selector, WriteSignal, clone, component, create_memo,
    create_selector, create_signal, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, ListRow, Scroll, Select, TextInput,
};
use beui::unstyled::ChoiceOption;
use beui::{Align, Direction, ItemSize, NodeId};

use crate::pane::{Pane, TerminalPane};
use crate::tasks::{PullRequest, Tasks};

const SPACING: f32 = 8.0;
const PRESETS: [&str; 7] = [
    "run //crates/block-app:app",
    "run //:check",
    "run //:verify",
    "run //crates/block-app:smoke",
    "build //crates/block-app:web",
    "run //crates/block-app:android -- --install",
    "run //crates/beui:demo-example",
];

#[derive(Clone)]
pub(crate) struct State {
    pub(crate) pull_requests: KeyedStore<String, PullRequest>,
    selected: ReadSignal<Option<String>>,
    set_selected: WriteSignal<Option<String>>,
    selection: Selector<Option<String>>,
    command: ReadSignal<String>,
    set_command: WriteSignal<String>,
    running: ReadSignal<bool>,
    pub(crate) set_running: WriteSignal<bool>,
    head: ReadSignal<String>,
    pub(crate) set_head: WriteSignal<String>,
    status: ReadSignal<String>,
    pub(crate) set_status: WriteSignal<String>,
    pub(crate) pane: Pane,
}

impl State {
    pub(crate) fn new(pane: Pane) -> Self {
        let (selected, set_selected) = create_signal(None::<String>);
        let selection = create_selector(clone!(selected -> move || selected.get()));
        let (command, set_command) = create_signal(PRESETS[0].to_owned());
        let (running, set_running) = create_signal(false);
        let (head, set_head) = create_signal(String::new());
        let (status, set_status) = create_signal("Listing pull requests".to_owned());
        Self {
            pull_requests: KeyedStore::new(),
            selected,
            set_selected,
            selection,
            command,
            set_command,
            running,
            set_running,
            head,
            set_head,
            status,
            set_status,
            pane,
        }
    }
}

#[component]
pub(crate) fn Launcher(state: State, tasks: Tasks) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=16.0>
            <PullRequestList
                @sizing=ItemSize::Percent(30.0)
                state={state.clone()}
                tasks={tasks.clone()}
            />
            <Controls @sizing=ItemSize::Percent(70.0) state tasks />
        </List>
    }
}

#[component]
fn PullRequestList(state: State, tasks: Tasks) -> NodeId {
    let refresh = clone!(state -> move || {
        state.set_status.set("Listing pull requests".to_owned());
        tasks.list_pull_requests();
    });
    let pull_requests = state.pull_requests.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Heading @sizing=ItemSize::Percent(100.0) content="Pull requests" />
                <Button label="Refresh" variant=ButtonVariant::Secondary on_click={refresh} />
            </List>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <ForEach keys={pull_requests.keys()}>
                    {move |branch: String| {
                        let pull_request = pull_requests.get(&branch);
                        let label = create_memo(move || pull_request.get().label);
                        let selected = state.selection.memo(Some(branch.clone()));
                        let set_selected = state.set_selected.clone();
                        view! {
                            <ListRow
                                selected
                                on_click={move || set_selected.set(Some(branch.clone()))}
                            >
                                <Body content={label} />
                            </ListRow>
                        }
                    }}
                </ForEach>
            </Scroll>
        </List>
    }
}

#[component]
fn Controls(state: State, tasks: Tasks) -> NodeId {
    let State {
        selected,
        command,
        set_command,
        running,
        set_running,
        head,
        status,
        pane,
        ..
    } = state;
    let cannot_switch = create_memo(clone!(selected running -> move || {
        running.get() || selected.with(Option::is_none)
    }));
    let cannot_run = create_memo(clone!(command running -> move || {
        running.get() || command.with(|command| command.trim().is_empty())
    }));
    let cannot_switch_and_run = create_memo(clone!(cannot_switch cannot_run -> move || {
        cannot_switch.get() || cannot_run.get()
    }));
    let idle = create_memo(clone!(running -> move || !running.get()));
    let arguments = clone!(command -> move || {
        command
            .get_untracked()
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let switch = clone!(selected set_running tasks -> move || {
        if let Some(branch) = selected.get_untracked() {
            set_running.set(true);
            tasks.run("switch", vec![branch]);
        }
    });
    let run = clone!(arguments set_running tasks -> move || {
        set_running.set(true);
        tasks.run("buck", arguments());
    });
    let switch_and_run = clone!(tasks -> move || {
        if let Some(branch) = selected.get_untracked() {
            set_running.set(true);
            let mut args = vec![branch];
            args.extend(arguments());
            tasks.run("switch", args);
        }
    });
    let stop = move || tasks.stop();
    view! {
        <List spacing=SPACING>
            <Heading content={head} />
            <Select
                options={view! {
                    <ForEach keys={PRESETS.to_vec()}>
                        {|label: &'static str| view! {
                            <ChoiceOption label />
                        }}
                    </ForEach>
                }}
                selected=Some(0)
                label="Preset"
                on_change={clone!(set_command -> move |selected: Option<usize>| {
                    if let Some(preset) = selected.and_then(|index| PRESETS.get(index)) {
                        set_command.set((*preset).to_owned());
                    }
                })}
            />
            <TextInput
                value={command}
                label="./scripts/buck arguments"
                on_change={move |value| set_command.set(value)}
            />
            <List direction=Direction::Horizontal spacing=SPACING>
                <Button
                    label="Switch"
                    variant=ButtonVariant::Secondary
                    disabled={cannot_switch}
                    on_click={switch}
                />
                <Button
                    label="Run"
                    variant=ButtonVariant::Secondary
                    disabled={cannot_run}
                    on_click={run}
                />
                <Button
                    label="Switch and run"
                    variant=ButtonVariant::Primary
                    disabled={cannot_switch_and_run}
                    on_click={switch_and_run}
                />
                <Button label="Stop" variant=ButtonVariant::Ghost disabled={idle} on_click={stop} />
            </List>
            <Caption content={status} />
            <TerminalPane @sizing=ItemSize::Percent(100.0) pane />
        </List>
    }
}
