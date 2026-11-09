//! The sandboxes: how old the list is, the key's standing, the filters, and a row per
//! sandbox with its start or stop.

use be_sdk_beui::beui::icons::{
    ICON_CIRCLE_PLAY, ICON_CIRCLE_STOP, ICON_CLOUD_OFF, ICON_REFRESH_CW, ICON_SAILBOAT,
    ICON_TRIANGLE_ALERT,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Align, Direction, NodeId, Overflow};

use super::state::{KeyStanding, State};
use super::ui::{SECTION_SPACING, SPACING};
use crate::api::{self, Action, BoatError, Phase, Sandbox};
use crate::time;

#[component]
pub(super) fn Sandboxes(state: State) -> NodeId {
    let listed = state.listed;
    let failure = state.failure;
    let now = state.now;
    let whose = create_memo(move || {
        let login = state
            .key
            .with(|key| key.as_ref().map(|key| key.login.clone()));
        let organization = state.filters.with(|filters| filters.organization.clone());
        match (login.unwrap_or_default(), organization.is_empty()) {
            (login, true) => login,
            (login, false) => format!("{organization}, as {login}"),
        }
    });
    let offline =
        create_memo(move || failure.with(|failure| matches!(failure, Some(BoatError::Offline(_)))));
    let status = create_memo(move || {
        let fetched = listed.with(|listed| listed.as_ref().map(|listed| listed.fetched));
        let failed = failure.with(|failure| match failure {
            Some(BoatError::Refused { message, .. }) => Some(message.clone()),
            _ => None,
        });
        let cut_off = listed.with(|listed| listed.as_ref().is_some_and(|listed| listed.cut_off));
        let status = match (fetched, offline.get(), failed) {
            (None, true, _) => "Offline, and no list was kept from before".to_owned(),
            (None, _, Some(message)) => message,
            (None, _, None) => "Reading the list...".to_owned(),
            (Some(fetched), true, _) => {
                format!("Offline: the list from {}", time::ago(fetched, now.get()))
            }
            (Some(fetched), false, Some(message)) => {
                format!("{message}. The list from {}", time::ago(fetched, now.get()))
            }
            (Some(fetched), false, None) => format!("Updated {}", time::ago(fetched, now.get())),
        };
        match cut_off {
            true => format!(
                "{status}. Only the first {} are shown",
                api::PAGES * api::PAGE
            ),
            false => status,
        }
    });
    let ids = create_memo(move || {
        listed.with(|listed| {
            listed
                .iter()
                .flat_map(|listed| listed.sandboxes.iter().map(|sandbox| sandbox.id.clone()))
                .collect::<Vec<_>>()
        })
    });
    let empty = create_memo(move || {
        listed.with(|listed| {
            listed
                .as_ref()
                .is_some_and(|listed| listed.sandboxes.is_empty())
        })
    });
    let notice = state.notice;
    let noticed = create_memo(move || notice.with(Option::is_some));
    let notice_text = create_memo(move || notice.get().unwrap_or_default());
    let rows = move |id: String| {
        view! {
            <SandboxRow state id />
        }
    };
    view! {
        <List spacing=SECTION_SPACING @test_id="boat.list">
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem grow=1.0>
                    <List spacing=Space::Xs>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text style=TextStyle::HEADING>
                                <Icon glyph=ICON_SAILBOAT />
                            </Text>
                            <Text style=TextStyle::HEADING>"boat.dev"</Text>
                        </List>
                        <Text style=TextStyle::CAPTION @test_id="boat.account">{whose}</Text>
                    </List>
                </ListItem>
                <Show condition={offline}>
                    <Text color=TextColor::Muted>
                        <Icon glyph=ICON_CLOUD_OFF />
                    </Text>
                </Show>
                <Text style=TextStyle::CAPTION @test_id="boat.status">{status}</Text>
                <Button
                    label="Refresh"
                    glyph=ICON_REFRESH_CW
                    icon_only=true
                    @test_id="boat.refresh"
                    on_click={move || state.refresh()}
                />
            </List>
            <KeyNotice state />
            <Show condition={noticed}>
                <Card>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <ListItem grow=1.0>
                            <Text color=TextColor::Danger @test_id="boat.notice">
                                {notice_text}
                            </Text>
                        </ListItem>
                        <Button
                            label="Dismiss"
                            variant=ButtonVariant::Ghost
                            @test_id="boat.notice.dismiss"
                            on_click={move || notice.set(None)}
                        />
                    </List>
                </Card>
            </Show>
            <Filters state />
            <Show condition={empty}>
                <List spacing=SPACING @test_id="boat.empty">
                    <Text>"No sandboxes"</Text>
                    <Text style=TextStyle::CAPTION>
                        "Sandboxes made with boat's command line or its dashboard show up here."
                    </Text>
                </List>
            </Show>
            <List spacing=SPACING>
                <ForEach keys={ids} view={rows} />
            </List>
        </List>
    }
}

/// Warns before the key expires, and offers Reconnect once boat no longer takes it.
#[component]
fn KeyNotice(state: State) -> NodeId {
    let standing = create_memo(move || state.standing());
    let shown = create_memo(move || {
        standing.with(|standing| !matches!(standing, None | Some(KeyStanding::Good)))
    });
    let message = create_memo(move || match standing.get() {
        Some(KeyStanding::Expiring(expires)) => format!(
            "This boat.dev key expires on {}. Make a new one and reconnect before then.",
            time::date(expires)
        ),
        Some(KeyStanding::Expired) => {
            "This boat.dev key has expired. Make a new one and reconnect.".to_owned()
        }
        Some(KeyStanding::Refused) => {
            "boat.dev no longer accepts this key. Make a new one and reconnect.".to_owned()
        }
        _ => String::new(),
    });
    let urgent = create_memo(move || {
        standing.with(|standing| !matches!(standing, Some(KeyStanding::Expiring(_))))
    });
    let color = create_memo(move || match urgent.get() {
        true => TextColor::Danger,
        false => TextColor::Warning,
    });
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <Card>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text color={color}>
                            <Icon glyph=ICON_TRIANGLE_ALERT />
                        </Text>
                        <ListItem grow=1.0>
                            <Text @test_id="boat.key-notice">{message}</Text>
                        </ListItem>
                        <Button
                            label="Reconnect"
                            variant=ButtonVariant::Primary
                            @test_id="boat.reconnect"
                            on_click={move || state.reconnecting.set(true)}
                        />
                    </List>
                </Card>
            </Show>
        </List>
    }
}

/// The block's filters, kept in its content once a field is left or submitted.
#[component]
fn Filters(state: State) -> NodeId {
    let read_only = state.editor.read_only();
    let prefix = create_memo(move || state.filters.with(|filters| filters.name_prefix.clone()));
    let label = create_memo(move || state.filters.with(|filters| filters.label.clone()));
    let prefix_draft = create_signal(prefix.get_untracked());
    let label_draft = create_signal(label.get_untracked());
    create_effect(move || prefix_draft.set(prefix.get()));
    create_effect(move || label_draft.set(label.get()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <ListItem grow=1.0>
                <TextInput
                    value={prefix_draft}
                    label="Name starts with"
                    placeholder="Any name"
                    disabled={read_only}
                    @test_id="boat.filter.prefix"
                    on_submit={move |value: String| state.set_name_prefix(value)}
                    on_focus_change={move |focused: bool| {
                        if !focused && prefix_draft.get_untracked() != prefix.get_untracked() {
                            state.set_name_prefix(prefix_draft.get_untracked());
                        }
                    }}
                />
            </ListItem>
            <ListItem grow=1.0>
                <TextInput
                    value={label_draft}
                    label="Label"
                    placeholder="key=value"
                    disabled={read_only}
                    @test_id="boat.filter.label"
                    on_submit={move |value: String| state.set_label(value)}
                    on_focus_change={move |focused: bool| {
                        if !focused && label_draft.get_untracked() != label.get_untracked() {
                            state.set_label(label_draft.get_untracked());
                        }
                    }}
                />
            </ListItem>
        </List>
    }
}

#[component]
fn SandboxRow(state: State, id: String) -> NodeId {
    let sandbox = {
        let id = id.clone();
        create_memo(move || {
            state.listed.with(|listed| {
                listed
                    .as_ref()
                    .and_then(|listed| listed.sandboxes.iter().find(|sandbox| sandbox.id == id))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    };
    let phase = create_memo(move || sandbox.with(|sandbox| state.phase(sandbox)));
    let name = create_memo(move || sandbox.with(|sandbox| sandbox.name.clone()));
    let detail = create_memo(move || sandbox.with(|sandbox| detail(sandbox, phase.get())));
    let confirming = create_signal(false);
    create_effect(move || {
        if phase.get() != Phase::Stopped {
            confirming.set(false);
        }
    });
    let startable = create_memo(move || phase.get() == Phase::Stopped && !confirming.get());
    let stoppable = create_memo(move || phase.get() == Phase::Running);
    let asking = create_memo(move || confirming.get() && phase.get() == Phase::Stopped);
    let question = create_memo(move || {
        format!(
            "Starting {} bills compute time until it stops. Start it?",
            name.get()
        )
    });
    // Starting and stopping use the viewer's own key and change nothing in the block, so a
    // read-only viewer may do them too.
    let disabled = create_memo(move || state.account().is_none());
    let id = store(id);
    let row_id = move |part: &str| id.with(|id| format!("boat.sandbox.{id}.{part}"));
    let start = move || {
        confirming.set(false);
        state.act(id.with(Clone::clone), Action::Resume);
    };
    let stop = move || state.act(id.with(Clone::clone), Action::Stop);
    view! {
        <Frame
            fill=ColorRole::Control
            radius=Radius::Md
            padding={Padding::symmetric(Space::Lg, Space::Md)}
        >
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <List spacing=Space::Xs>
                            <Text weight=Weight::Bold overflow=Overflow::Ellipsis>{name}</Text>
                            <Text style=TextStyle::CAPTION @test_id={row_id("detail")}>
                                {detail}
                            </Text>
                        </List>
                    </ListItem>
                    <Show condition={startable}>
                        <Button
                            label="Start"
                            glyph=ICON_CIRCLE_PLAY
                            disabled={disabled}
                            @test_id={row_id("start")}
                            on_click={move || confirming.set(true)}
                        />
                    </Show>
                    <Show condition={stoppable}>
                        <Button
                            label="Stop"
                            glyph=ICON_CIRCLE_STOP
                            disabled={disabled}
                            @test_id={row_id("stop")}
                            on_click={stop}
                        />
                    </Show>
                </List>
                <Show condition={asking}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <ListItem grow=1.0>
                            <Text style=TextStyle::CAPTION>{question}</Text>
                        </ListItem>
                        <Button
                            label="Start"
                            variant=ButtonVariant::Primary
                            @test_id={row_id("confirm")}
                            on_click={start}
                        />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Ghost
                            @test_id={row_id("cancel")}
                            on_click={move || confirming.set(false)}
                        />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

/// `Running · default, 4 vCPU, 8 GB · 2001:db8::23`, or why it failed.
fn detail(sandbox: &Sandbox, phase: Phase) -> String {
    let mut parts = vec![phase.label().to_owned()];
    if phase == Phase::Failed
        && let Some(error) = sandbox.error.as_ref().filter(|error| !error.is_empty())
    {
        parts.push(error.clone());
        return parts.join(" · ");
    }
    let size = [
        sandbox.size.clone(),
        sandbox.vcpu.map(|vcpu| format!("{vcpu} vCPU")),
        sandbox.memory_gb.map(|memory| format!("{memory} GB")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !size.is_empty() {
        parts.push(size.join(", "));
    }
    if phase == Phase::Running
        && let Some(address) = sandbox.ssh_endpoint.clone().or_else(|| sandbox.ip.clone())
    {
        parts.push(address);
    }
    parts.join(" · ")
}
