use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use block::BlockReferenceList;
use block_client::blocks::version_control_data::{CommitId, VersionControlData};
use block_client::blocks::version_control_worktree::VersionControlWorktree;
use block_editor_plugin::beui::icons::{
    ICON_ALT_ROUTE, ICON_CHECK_CIRCLE, ICON_REFRESH, ICON_SYNC, ICON_WARNING,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef, ReadSignal, Show, clone,
    component, create_effect, create_memo, create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Code, Heading, Icon, IconButton, Scroll, Separator,
    TextInput, use_theme,
};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockLink, ChildTarget, Editor, Sidebar};
use uuid::Uuid;

use super::tasks::Work;

const PADDING: f32 = 16.0;
const ROW_SPACING: f32 = 8.0;
const SECTION_SPACING: f32 = 12.0;
const INTRINSIC_WIDTH: f32 = 480.0;
const MEMBER_ROW_HEIGHT: f32 = 24.0;

#[derive(Clone, PartialEq)]
struct Branch {
    name: String,
    head: CommitId,
}

#[component]
pub fn WorktreeView(editor: Editor) -> NodeId {
    let worktree = editor.block::<VersionControlWorktree>();
    let work = Work::new(&editor);
    let members = worktree.project(|worktree| {
        worktree
            .members()
            .map(|(_, live_id)| live_id)
            .collect::<Vec<Uuid>>()
    });
    let checked_out = worktree.project(|worktree| Some(worktree.checked_out_commit().clone()));
    let repo = worktree.project(|worktree| Some(worktree.repo()));
    let types = watch_types(&editor);

    let sized = editor.clone();
    create_effect(clone!(members -> move || {
        let rows = members.with(Vec::len).max(1);
        let height = MEMBER_ROW_HEIGHT * rows as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let panel = editor.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <Members editor={editor} members={members} types={types} />
                </Frame>
                <Sidebar shown={chrome}>
                    <WorktreePanel
                        @sizing=ItemSize::Percent(100.0)
                        editor={panel}
                        work={work}
                        repo={repo}
                        checked_out={checked_out}
                    />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn Members(
    editor: Editor,
    members: ReadSignal<Vec<Uuid>>,
    types: Memo<HashMap<Uuid, Uuid>>,
) -> NodeId {
    let keys = create_memo(clone!(members -> move || members.get()));
    let empty = create_memo(clone!(members -> move || members.with(Vec::is_empty)));
    view! {
        <List spacing=ROW_SPACING>
            <Show condition={empty}>
                <Caption content="This worktree has no content yet." />
            </Show>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <List spacing=4.0>
                    <ForEach keys={keys}>
                        {move |id: Uuid| {
                            let target = create_memo(clone!(types id -> move || {
                                types.with(|types| {
                                    types.get(&id).map(|kind| ChildTarget::new(id, *kind))
                                })
                            }));
                            view! {
                                <BlockLink
                                    editor={editor.clone()}
                                    block={target}
                                    @test_id={format!("worktree.member.{id}")}
                                />
                            }
                        }}
                    </ForEach>
                </List>
            </Scroll>
        </List>
    }
}

#[component]
fn WorktreePanel(
    editor: Editor,
    work: Rc<Work>,
    repo: ReadSignal<Option<Uuid>>,
    checked_out: ReadSignal<Option<CommitId>>,
) -> NodeId {
    let branches = branches_of(&editor, repo);
    view! {
        <List spacing=SECTION_SPACING>
            <StatusPanel
                editor={editor}
                work={Rc::clone(&work)}
                checked_out={checked_out.clone()}
            />
            <Separator />
            <BranchPanel work={work} branches={branches} checked_out={checked_out} />
        </List>
    }
}

#[component]
fn StatusPanel(
    editor: Editor,
    work: Rc<Work>,
    checked_out: ReadSignal<Option<CommitId>>,
) -> NodeId {
    let (message, set_message) = create_signal(String::new());
    let dirty = work.dirty();
    let checking = work.checking();
    let committing = work.committing();
    let switching = work.switching();
    let error = work.error();
    let read_only = editor.read_only();

    let label = create_memo(clone!(dirty -> move || match dirty.get() {
        None => "Checking status…".to_owned(),
        Some(true) => "Uncommitted changes".to_owned(),
        Some(false) => "Clean".to_owned(),
    }));
    let glyph = create_memo(clone!(dirty -> move || match dirty.get() {
        None => ICON_SYNC.to_owned(),
        Some(true) => ICON_WARNING.to_owned(),
        Some(false) => ICON_CHECK_CIRCLE.to_owned(),
    }));
    let blocked = create_memo(clone!(dirty read_only -> move || {
        read_only.get() || dirty.get() != Some(true)
    }));
    let cannot_commit = create_memo(clone!(blocked committing message -> move || {
        blocked.get() || committing.get() || message.with(|message| message.trim().is_empty())
    }));
    let cannot_discard =
        create_memo(clone!(blocked switching -> move || blocked.get() || switching.get()));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));

    let author = editor.client().account_id();
    let refresh = clone!(work -> move || work.refresh());
    let commit = clone!(work message set_message -> move || {
        work.commit(author, message.get_untracked());
        set_message.set(String::new());
    });
    let discard = clone!(work checked_out -> move || {
        if let Some(commit) = checked_out.get_untracked() {
            work.switch(commit, true);
        }
    });
    let theme = use_theme();
    view! {
        <List spacing=ROW_SPACING>
            <Heading content="Status" />
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Icon glyph={glyph} color={theme.text_muted.clone()} />
                <Body @sizing=ItemSize::Percent(100.0) content={label} />
                <IconButton
                    glyph={ICON_REFRESH.to_owned()}
                    label="Refresh status"
                    disabled={checking}
                    @test_id={"worktree.refresh"}
                    on_click={refresh}
                />
            </List>
            <TextInput
                value={message}
                placeholder="Commit message"
                label="Commit message"
                disabled={read_only}
                @test_id={"worktree.commit-message"}
                on_change={move |value| set_message.set(value)}
            />
            <Button
                label="Commit"
                variant=ButtonVariant::Primary
                disabled={cannot_commit}
                @test_id={"worktree.commit"}
                on_click={commit}
            />
            <Button
                label="Discard changes"
                variant=ButtonVariant::Secondary
                disabled={cannot_discard}
                @test_id={"worktree.discard"}
                on_click={discard}
            />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}

#[component]
fn BranchPanel(
    work: Rc<Work>,
    branches: Memo<Vec<Branch>>,
    checked_out: ReadSignal<Option<CommitId>>,
) -> NodeId {
    let keys = create_memo(clone!(branches -> move || {
        branches.with(|branches| {
            branches.iter().map(|branch| branch.name.clone()).collect::<Vec<String>>()
        })
    }));
    let loading = create_memo(clone!(branches -> move || branches.with(Vec::is_empty)));
    let awaiting = work.awaiting();
    let blocked = create_memo(clone!(awaiting -> move || awaiting.get().is_some()));
    let switching = work.switching();
    let confirm = clone!(work awaiting -> move || {
        if let Some(target) = awaiting.get_untracked() {
            work.switch(target, true);
        }
    });
    let cancel = clone!(work -> move || work.dismiss());
    let rows = Rc::clone(&work);
    let theme = use_theme();
    view! {
        <List spacing=ROW_SPACING>
            <Heading content="Branches" />
            <Show condition={loading}>
                <Caption content="Loading repository…" />
            </Show>
            <ForEach keys={keys}>
                {move |name: String| {
                    let head = create_memo(clone!(branches name -> move || {
                        branches.with(|branches| {
                            branches
                                .iter()
                                .find(|branch| branch.name == name)
                                .map(|branch| branch.head.clone())
                        })
                    }));
                    view! {
                        <BranchRow
                            work={Rc::clone(&rows)}
                            name={name}
                            head={head}
                            checked_out={checked_out.clone()}
                            switching={switching.clone()}
                        />
                    }
                }}
            </ForEach>
            <Show condition={blocked}>
                <List spacing=ROW_SPACING>
                    <Caption
                        content="This worktree has uncommitted changes."
                        color={theme.warning.clone()}
                    />
                    <Button
                        label="Discard and switch"
                        variant=ButtonVariant::Primary
                        @test_id={"worktree.discard-and-switch"}
                        on_click={confirm}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        @test_id={"worktree.cancel-switch"}
                        on_click={cancel}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn BranchRow(
    work: Rc<Work>,
    name: String,
    head: Memo<Option<CommitId>>,
    checked_out: ReadSignal<Option<CommitId>>,
    switching: Memo<bool>,
) -> NodeId {
    let short = create_memo(clone!(head -> move || {
        head.with(|head| head.as_ref().map_or_else(String::new, CommitId::short))
    }));
    let here = create_memo(clone!(head checked_out -> move || {
        head.with(|head| checked_out.with(|checked_out| head == checked_out && head.is_some()))
    }));
    let elsewhere = create_memo(clone!(here -> move || !here.get()));
    let switch = clone!(work head -> move || {
        if let Some(head) = head.get_untracked() {
            work.switch(head, false);
        }
    });
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Icon glyph={ICON_ALT_ROUTE.to_owned()} color={theme.text_muted.clone()} />
            <Body content={name.clone()} />
            <Code @sizing=ItemSize::Percent(100.0) content={short} />
            <Show condition={here}>
                <Caption content="checked out" />
            </Show>
            <Show condition={elsewhere}>
                <Button
                    label="Switch"
                    variant=ButtonVariant::Secondary
                    disabled={switching}
                    @test_id={format!("worktree.switch.{name}")}
                    on_click={switch}
                />
            </Show>
        </List>
    }
}

fn branches_of(editor: &Editor, repo: ReadSignal<Option<Uuid>>) -> Memo<Vec<Branch>> {
    let (branches, set_branches) = create_signal(Vec::<Branch>::new());
    let client = Arc::clone(editor.client());
    editor.each_frame(move || {
        let Some(repo) = repo.get_untracked() else {
            return;
        };
        let handle = client.get_block::<VersionControlData>(repo);
        let Some(data) = handle.read() else {
            return;
        };
        set_branches.set(
            data.branches()
                .iter()
                .map(|(name, head)| Branch {
                    name: name.clone(),
                    head: head.clone(),
                })
                .collect(),
        );
    });
    create_memo(move || branches.get())
}

fn watch_types(editor: &Editor) -> Memo<HashMap<Uuid, Uuid>> {
    let references = editor
        .client()
        .watch_references(BlockReferenceList::References(editor.block_id()));
    let (types, set_types) = create_signal(HashMap::<Uuid, Uuid>::new());
    editor.each_frame(move || {
        set_types.set(
            references
                .read()
                .into_iter()
                .map(|reference| (reference.id, reference.block_type))
                .collect(),
        );
    });
    create_memo(move || types.get())
}
