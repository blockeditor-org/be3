use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use be_sdk_beui::beui::datetime::DateTime;
use be_sdk_beui::beui::{NodeId, dock::TabId, reactive::*, styled::*};
use be_sdk_beui::{ChildTarget, Editor, Pushed, answer};
use be_sdk_beui::{Revision, RevisionPreview};
use uuid::Uuid;

use be_sdk_shell::{TabItem, Workspace};
use beui::Direction;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Selected {
    block: Uuid,
    commit: [u8; 32],
    preview: Uuid,
    time: i64,
}

/// The revisions listed, with the block they are of.
type Listed = Option<(Uuid, Vec<Revision>)>;

#[derive(Clone, Copy)]
pub(crate) struct RevisionHistory {
    editor: Editor,
    target: Signal<Option<TabItem>>,
    /// The target's revisions as the host lists them, again whenever its head moves: `None`
    /// while there is no target. It keeps the last listing while it lists again.
    listing: AsyncMemo<Listed>,
    /// The last listing that arrived, kept while a later one fails for the same block.
    listed: Memo<Listed>,
    selected: Signal<Option<Selected>>,
    busy: Signal<bool>,
    confirm: Signal<bool>,
    wanted: Signal<Option<(Uuid, [u8; 32])>>,
    state: Stored<Requests>,
}

#[derive(Default)]
struct Requests {
    generation: Cell<u64>,
    /// The preview of the selected revision, closed when dropped.
    preview: RefCell<Option<RevisionPreview>>,
}

impl RevisionHistory {
    pub(crate) fn new(editor: Editor) -> Self {
        let target = create_signal(None::<TabItem>);
        create_effect(move || {
            if let Some(item) = target.get() {
                editor.host().watch_history([item.id]);
            }
        });
        let changed = editor.pushed(Pushed::Histories);
        let host = editor.host();
        let head = create_memo(move || {
            changed.track();
            target
                .get()
                .map(|item| (item.id, host.history(item.id).head))
        });
        let host = editor.host();
        let listing = create_async_memo(move || {
            let asked = head
                .get()
                .map(|(block, _)| (block, host.list_revisions(block)));
            async move {
                let Some((block, asked)) = asked else {
                    return Ok(None);
                };
                Ok(Some((block, answer(asked).await?)))
            }
        });
        let listed = create_memo(move |last: Option<&Listed>| {
            let shown = target.with(|target| target.as_ref().map(|item| item.id));
            listing.with(|listing| match listing {
                Ok(listed) => listed.clone(),
                Err(Pending::Loading) => last.cloned().flatten(),
                Err(Pending::Failed(_)) => last
                    .cloned()
                    .flatten()
                    .filter(|(block, _)| Some(*block) == shown),
            })
        });
        let history = Self {
            editor,
            target,
            listing,
            listed,
            selected: create_signal(None),
            busy: create_signal(false),
            confirm: create_signal(false),
            wanted: create_signal(None),
            state: store(Requests::default()),
        };
        // A revision asked for before its block was listed is shown once it is.
        create_effect(move || {
            let Some((block, commit)) = history.wanted.get() else {
                return;
            };
            let found = listed.with(|listed| match listed {
                Some((listed, entries)) if *listed == block => {
                    Some(entries.iter().find(|entry| entry.commit == commit).cloned())
                }
                _ => None,
            });
            if let Some(found) = found {
                untrack(|| {
                    history.wanted.set(None);
                    if let Some(revision) = found {
                        history.choose(revision);
                    }
                });
            }
        });
        history
    }

    pub(crate) fn target(self) -> Option<TabItem> {
        self.target.get_untracked()
    }

    pub(crate) fn follow(self, target: Option<TabItem>) {
        if self.target.get_untracked() == target {
            return;
        }
        self.current();
        self.target.set(target);
    }

    /// Previews `commit` of `block` once the history lists it.
    pub(crate) fn show_at(self, block: Uuid, commit: [u8; 32]) {
        self.wanted.set(Some((block, commit)));
    }

    fn choose(self, revision: Revision) {
        let Some(item) = self.target.get_untracked() else {
            return;
        };
        self.current();
        let state = self.state.value();
        state.generation.set(state.generation.get() + 1);
        self.busy.set(true);
        let generation = state.generation.get();
        let block = item.id;
        let asked = self.editor.host().preview_revision(block, revision.commit);
        self.editor.then(asked, move |result| {
            if !self.state.is_alive() || generation != self.state.value().generation.get() {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(preview) => {
                    self.selected.set(Some(Selected {
                        block,
                        commit: revision.commit,
                        preview: preview.block,
                        time: revision.time,
                    }));
                    self.state.value().preview.replace(Some(preview));
                }
                Err(error) => self
                    .editor
                    .host()
                    .report_failure(format!("The revision could not be shown. {error}")),
            }
        });
    }

    pub(crate) fn current(self) {
        let state = self.state.value();
        state.generation.set(state.generation.get() + 1);
        self.busy.set(false);
        if self.selected.get_untracked().is_some() {
            self.selected.set(None);
        }
        state.preview.take();
        self.confirm.set(false);
    }

    fn restore(self) {
        let Some(selected) = self.selected.get_untracked() else {
            return;
        };
        self.busy.set(true);
        self.confirm.set(false);
        let generation = self.state.value().generation.get();
        let asked = self
            .editor
            .host()
            .restore_revision(selected.block, selected.commit);
        self.editor.then(asked, move |result| {
            if !self.state.is_alive() || generation != self.state.value().generation.get() {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(()) => {
                    self.current();
                    self.listing.refresh();
                }
                Err(error) => self
                    .editor
                    .host()
                    .report_failure(format!("The revision could not be restored. {error}")),
            }
        });
    }

    pub(crate) fn preview_for(self, item: Option<TabItem>) -> Option<ChildTarget> {
        let item = item?;
        let selected = self.selected.get()?;
        (selected.block == item.id)
            .then(|| ChildTarget::new(selected.preview, item.block_type).edited_with(item.editor))
    }
}

fn revision_time(time: i64) -> String {
    let seconds = time.div_euclid(1000);
    format!(
        "{}:{:02} UTC",
        DateTime::from_unix(seconds).iso(),
        seconds.rem_euclid(60)
    )
}

fn revision_key(commit: &[u8; 32]) -> String {
    commit.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[component]
pub(crate) fn RevisionHistoryPanel(workspace: Workspace, history: RevisionHistory) -> NodeId {
    let entries = create_memo(move || {
        history.listed.with(|listed| {
            listed
                .as_ref()
                .map(|(_, entries)| entries.clone())
                .unwrap_or_default()
        })
    });
    let revisions = create_memo(move || {
        entries.with(|entries| {
            entries
                .iter()
                .map(|entry| (entry.commit, *entry))
                .collect::<HashMap<_, _>>()
        })
    });
    let keys = create_memo(move || {
        entries.with(|entries| entries.iter().map(|entry| entry.commit).collect::<Vec<_>>())
    });
    let no_file = create_memo(move || history.target.get().is_none());
    // Whether the revisions shown are of another block than the target, or none.
    let elsewhere = create_memo(move || {
        history
            .listed
            .with(|listed| listed.as_ref().map(|(block, _)| *block))
            != history.target.get().map(|item| item.id)
    });
    let stale = create_memo(move || history.busy.get() || elsewhere.get());
    // A failure shows until the history lists again.
    let error = create_memo(move || {
        if history.listing.pending() {
            return None;
        }
        history.listing.with(|listing| match listing {
            Err(Pending::Failed(error)) => Some(error.clone()),
            _ => None,
        })
    });
    let name = create_memo(move || {
        history
            .target
            .get()
            .map(|item| workspace.label(item.id, item.block_type).name)
            .unwrap_or_default()
    });
    // The listing has its own boundary, so the workspace never waits for it: before the first
    // listing arrives it says so here.
    let fallback = move |pending: Pending| match pending {
        Pending::Loading => view! {
            <Text style=TextStyle::CAPTION>"Loading revisions…"</Text>
        },
        Pending::Failed(error) => view! {
            <ListingFailed history error />
        },
    };
    view! {
        <Scroll @test_id={"workspace.revisions"}>
            <Frame padding=Space::Lg>
                <List spacing=Space::Md>
                    <Text style=TextStyle::HEADING>"Revision history"</Text>
                    <Show
                        value={no_file}
                        fallback={move || view! {
                            <Text>{name}</Text>
                        }}
                    >
                        <Text style=TextStyle::CAPTION>
                            "Open a file to see its revision history."
                        </Text>
                    </Show>
                    <Loading fallback>||
                        <Show value={move || history.listing.pending() && elsewhere.get()}>
                            <Text style=TextStyle::CAPTION>"Loading revisions…"</Text>
                        </Show>
                        <Show value={history.busy}>
                            <Text style=TextStyle::CAPTION>"Working…"</Text>
                        </Show>
                        <Show
                            value={move || {
                                !no_file.get()
                                    && !history.listing.pending()
                                    && error.with(Option::is_none)
                                    && entries.with(Vec::is_empty)
                            }}
                        >
                            <Text style=TextStyle::CAPTION>"No saved revisions yet."</Text>
                        </Show>
                        <Show value={error}>|error: ReadSignal<String>|
                            <ListingFailed history error />
                        </Show>
                        <ForEach value={keys}>|commit: [u8; 32]| {
                            let label = create_memo(move || revisions.with(|entries| entries.get(&commit).map(|entry| {
                                let date = revision_time(entry.time);
                                format!("{date}{}", if history.selected.get().is_some_and(|selected| selected.commit == commit) { " · Preview" } else if entry.current { " · Current" } else { "" })
                            }).unwrap_or_default()));
                            view! {
                                <Button
                                    label
                                    variant=ButtonVariant::Secondary
                                    disabled={stale}
                                    @test_id={format!("workspace.revision.{}", revision_key(&commit))}
                                    on_click={move || {
                                        let revision = revisions.with(|entries| entries.get(&commit).cloned());
                                        if let Some(revision) = revision {
                                            history.choose(revision);
                                            if workspace.phone().get_untracked() && let Some(item) = history.target.get_untracked()
                                                && let Some(tab) = workspace.tab_showing(item.id) {
                                                workspace.layout().show(&tab);
                                            }
                                        }
                                    }}
                                />
                            }
                        }</ForEach>
                    </Loading>
                </List>
            </Frame>
        </Scroll>
    }
}

/// Why the history could not be listed, with a way to ask again.
#[component]
fn ListingFailed(history: RevisionHistory, error: Prop<String>) -> NodeId {
    view! {
        <List spacing=Space::Md>
            <Text style=TextStyle::CAPTION>{error}</Text>
            <Button
                variant=ButtonVariant::Secondary
                label="Retry"
                @test_id={"workspace.revisions.retry"}
                on_click={move || history.listing.refresh()}
            />
        </List>
    }
}

#[component]
pub(crate) fn RevisionPreviewBar(
    history: RevisionHistory,
    tab: TabId,
    workspace: Workspace,
) -> NodeId {
    let visible = create_memo(move || history.preview_for(workspace.tab(tab)).is_some());
    let label = create_memo(move || {
        history
            .selected
            .get()
            .map(|selected| format!("Revision from {} · Read only", revision_time(selected.time)))
            .unwrap_or_default()
    });
    view! {
        <List spacing=Space::None>
            <Show value={visible}>
                <Frame padding=Space::Lg>
                    <List spacing=Space::Md>
                        <Text style=TextStyle::CAPTION>{label}</Text>
                        <List direction=Direction::Horizontal spacing=Space::Md>
                            <Button
                                label="Back to current"
                                variant=ButtonVariant::Secondary
                                disabled={history.busy}
                                @test_id={"workspace.revision.current"}
                                on_click={move || history.current()}
                            />
                            <Show
                                value={move || {
                                    history
                                        .target
                                        .get()
                                        .is_some_and(|item| workspace.can_edit(item.id))
                                }}
                            >
                                <Button
                                    variant=ButtonVariant::Primary
                                    label="Restore this revision"
                                    disabled={history.busy}
                                    @test_id={"workspace.revision.restore"}
                                    on_click={move || history.confirm.set(true)}
                                />
                            </Show>
                        </List>
                        <Show value={history.confirm}>
                            <List spacing=Space::Md>
                                <Text style=TextStyle::CAPTION>
                                    "Restore this file as a new revision? Current edits will be saved. Contained and linked files will stay unchanged."
                                </Text>
                                <List direction=Direction::Horizontal spacing=Space::Md>
                                    <Button
                                        variant=ButtonVariant::Primary
                                        label="Restore"
                                        @test_id={"workspace.revision.confirm"}
                                        on_click={move || history.restore()}
                                    />
                                    <Button
                                        label="Cancel"
                                        variant=ButtonVariant::Secondary
                                        @test_id={"workspace.revision.cancel"}
                                        on_click={move || history.confirm.set(false)}
                                    />
                                </List>
                            </List>
                        </Show>
                    </List>
                </Frame>
            </Show>
        </List>
    }
}
