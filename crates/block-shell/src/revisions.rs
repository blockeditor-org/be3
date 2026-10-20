use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use block_editor_beui::Revision;
use block_editor_beui::beui::{NodeId, reactive::*, styled::*, unstyled::TabId};
use block_editor_beui::block_ui::datetime::format_datetime_utc;
use block_editor_beui::{ChildTarget, Editor, Pushed};
use uuid::Uuid;

use super::{Workspace, tab::TabItem};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Selected {
    block: Uuid,
    commit: [u8; 32],
    preview: Uuid,
    time: i64,
}

#[derive(Clone, Copy)]
pub(crate) struct RevisionHistory {
    editor: Editor,
    target: Signal<Option<TabItem>>,
    entries: Signal<Vec<Revision>>,
    selected: Signal<Option<Selected>>,
    error: Signal<Option<String>>,
    loading: Signal<bool>,
    busy: Signal<bool>,
    confirm: Signal<bool>,
    state: Stored<Pending>,
}

#[derive(Default)]
struct Pending {
    generation: Cell<u64>,
    latest_list: Cell<u64>,
    lists: RefCell<Vec<(u64, u64)>>,
    previews: RefCell<Vec<(u64, u64, Uuid, Revision)>>,
    restores: RefCell<Vec<(u64, u64)>>,
}

impl RevisionHistory {
    pub(crate) fn new(editor: Editor) -> Self {
        let target = create_signal(None);
        let entries = create_signal(Vec::new());
        let selected = create_signal(None);
        let error = create_signal(None);
        let loading = create_signal(false);
        let busy = create_signal(false);
        let confirm = create_signal(false);
        let history = Self {
            editor,
            target,
            entries,
            selected,
            error,
            loading,
            busy,
            confirm,
            state: store(Pending::default()),
        };
        let changed = editor.pushed(Pushed::Histories);
        let previous = Cell::new(None);
        create_effect(move || {
            changed.get();
            let head = history.target.get().map(|item| {
                editor.host().watch_history([item.id]);
                (item.id, editor.host().history(item.id).head)
            });
            if previous.replace(head) != head {
                untrack(|| history.load());
            }
        });
        editor.on_reply(move || {
            if history.state.is_alive() {
                history.replied();
            }
        });
        on_cleanup(move || {
            if let Some(selected) = history.selected.get_untracked() {
                editor.host().release_revision(selected.preview);
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
        self.state
            .value()
            .generation
            .set(self.state.value().generation.get() + 1);
        self.busy.set(false);
        self.entries.set(Vec::new());
        self.error.set(None);
        self.target.set(target);
    }

    fn load(self) {
        let Some(item) = self.target.get_untracked() else {
            self.loading.set(false);
            return;
        };
        self.loading.set(true);
        self.error.set(None);
        let state = self.state.value();
        let request = self.editor.host().list_revisions(item.id);
        state.latest_list.set(request);
        state
            .lists
            .borrow_mut()
            .push((request, state.generation.get()));
    }

    fn choose(self, revision: Revision) {
        let Some(item) = self.target.get_untracked() else {
            return;
        };
        self.current();
        let state = self.state.value();
        state.generation.set(state.generation.get() + 1);
        self.busy.set(true);
        self.error.set(None);
        let request = self
            .editor
            .host()
            .preview_revision(item.id, revision.commit);
        state
            .previews
            .borrow_mut()
            .push((request, state.generation.get(), item.id, revision));
    }

    pub(crate) fn current(self) {
        let state = self.state.value();
        state.generation.set(state.generation.get() + 1);
        self.loading.set(false);
        self.busy.set(false);
        if let Some(selected) = self.selected.get_untracked() {
            self.selected.set(None);
            self.editor.host().release_revision(selected.preview);
        }
        self.confirm.set(false);
    }

    fn restore(self) {
        let Some(selected) = self.selected.get_untracked() else {
            return;
        };
        self.busy.set(true);
        self.confirm.set(false);
        self.error.set(None);
        let request = self
            .editor
            .host()
            .restore_revision(selected.block, selected.commit);
        self.state
            .value()
            .restores
            .borrow_mut()
            .push((request, self.state.value().generation.get()));
    }

    fn replied(self) {
        let state = self.state.value();
        state.lists.borrow_mut().retain(|(request, generation)| {
            let Some(result) = self.editor.host().take_revisions(*request) else {
                return true;
            };
            if *generation == state.generation.get() && *request == state.latest_list.get() {
                self.loading.set(false);
                match result {
                    Ok(entries) => self.entries.set(entries),
                    Err(error) => self.error.set(Some(error)),
                }
            }
            false
        });
        state
            .previews
            .borrow_mut()
            .retain(|(request, generation, block, revision)| {
                let Some(result) = self.editor.host().take_revision_preview(*request) else {
                    return true;
                };
                if *generation != state.generation.get() {
                    if let Ok(preview) = result {
                        self.editor.host().release_revision(preview);
                    }
                    return false;
                }
                self.busy.set(false);
                match result {
                    Ok(preview) => self.selected.set(Some(Selected {
                        block: *block,
                        commit: revision.commit,
                        preview,
                        time: revision.time,
                    })),
                    Err(error) => self.error.set(Some(error)),
                }
                false
            });
        let mut reload = false;
        state.restores.borrow_mut().retain(|(request, generation)| {
            let Some(result) = self.editor.host().take_revision_restore(*request) else {
                return true;
            };
            if *generation == state.generation.get() {
                self.busy.set(false);
                match result {
                    Ok(()) => {
                        self.current();
                        reload = true;
                    }
                    Err(error) => self.error.set(Some(error)),
                }
            }
            false
        });
        if reload {
            self.load();
        }
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
        format_datetime_utc(seconds),
        seconds.rem_euclid(60)
    )
}

pub fn revision_key(commit: &[u8; 32]) -> String {
    commit.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[component]
pub fn RevisionHistoryPanel(workspace: Workspace) -> NodeId {
    let history = workspace.revisions;
    let revisions = create_memo(move || {
        history.entries.with(|entries| {
            entries
                .iter()
                .map(|entry| (entry.commit, entry.clone()))
                .collect::<HashMap<_, _>>()
        })
    });
    let keys = create_memo(move || {
        history
            .entries
            .with(|entries| entries.iter().map(|entry| entry.commit).collect::<Vec<_>>())
    });
    let no_file = create_memo(move || history.target.get().is_none());
    let has_file = create_memo(move || !no_file.get());
    let empty = create_memo(move || {
        !no_file.get()
            && !history.loading.get()
            && history.error.get().is_none()
            && history.entries.with(Vec::is_empty)
    });
    let error = create_memo(move || history.error.get().unwrap_or_default());
    let failed = create_memo(move || history.error.get().is_some());
    let name = create_memo(move || {
        history
            .target
            .get()
            .map(|item| workspace.label(item.id, item.block_type).name)
            .unwrap_or_default()
    });
    view! {
        <Scroll @test_id={"workspace.revisions"}>
            <Frame padding=14.0>
                <List spacing=8.0>
                    <Text wrap=true style=TextStyle::HEADING string="Revision history" />
                    <Show condition={no_file}>
                        <Text
                            wrap=true
                            style=TextStyle::CAPTION
                            string="Open a file to see its revision history."
                        />
                    </Show>
                    <Show condition={has_file}>
                        <Text wrap=true string={name} />
                    </Show>
                    <Show condition={history.loading}>
                        <Text wrap=true style=TextStyle::CAPTION string="Loading revisions…" />
                    </Show>
                    <Show condition={history.busy}>
                        <Text wrap=true style=TextStyle::CAPTION string="Working…" />
                    </Show>
                    <Show condition={empty}>
                        <Text wrap=true style=TextStyle::CAPTION string="No saved revisions yet." />
                    </Show>
                    <Show condition={failed}>
                        <List spacing=8.0>
                            <Text wrap=true style=TextStyle::CAPTION string={error} />
                            <Button
                                variant=ButtonVariant::Secondary
                                label="Retry"
                                @test_id={"workspace.revisions.retry"}
                                on_click={move || history.load()}
                            />
                        </List>
                    </Show>
                    <ForEach keys={keys}>|commit: [u8; 32]| {
                        let label = create_memo(move || revisions.with(|entries| entries.get(&commit).map(|entry| {
                            let date = revision_time(entry.time);
                            format!("{date}{}", if history.selected.get().is_some_and(|selected| selected.commit == commit) { " · Preview" } else if entry.current { " · Current" } else { "" })
                        }).unwrap_or_default()));
                        view! {
                            <Button
                                label
                                variant=ButtonVariant::Secondary
                                disabled={history.busy}
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
                </List>
            </Frame>
        </Scroll>
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
    let failed = create_memo(move || history.error.get().is_some());
    let error = create_memo(move || history.error.get().unwrap_or_default());
    let can_restore = create_memo(move || {
        history
            .target
            .get()
            .is_some_and(|item| workspace.can_edit(item.id))
    });
    view! {
        <List spacing=0.0>
            <Show condition={visible}>
                <Frame padding=14.0>
                    <List spacing=8.0>
                        <Text wrap=true style=TextStyle::CAPTION string={label} />
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Button
                                label="Back to current"
                                variant=ButtonVariant::Secondary
                                disabled={history.busy}
                                @test_id={"workspace.revision.current"}
                                on_click={move || history.current()}
                            />
                            <Show condition={can_restore}>
                                <Button
                                    variant=ButtonVariant::Primary
                                    label="Restore this revision"
                                    disabled={history.busy}
                                    @test_id={"workspace.revision.restore"}
                                    on_click={move || history.confirm.set(true)}
                                />
                            </Show>
                        </List>
                        <Show condition={failed}>
                            <Text wrap=true style=TextStyle::CAPTION string={error} />
                        </Show>
                        <Show condition={history.confirm}>
                            <List spacing=8.0>
                                <Text
                                    wrap=true
                                    style=TextStyle::CAPTION
                                    string="Restore this file as a new revision? Current edits will be saved. Contained and linked files will stay unchanged."
                                />
                                <List direction=Direction::Horizontal spacing=8.0>
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
