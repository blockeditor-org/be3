use block::Block;
use block_client::blocks::version_control_data::{
    Commit, CommitId, MAIN_BRANCH, VersionControlData, VersionControlDataOperation,
};
use block_client::blocks::version_control_worktree::VersionControlWorktree;
use block_editor_plugin::Editor;
use block_editor_plugin::beui::icons::{ICON_ALT_ROUTE, ICON_COMMIT, ICON_PERSON, ICON_SCHEDULE};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Scroll, Selector, Show, Spacer,
    WriteSignal, clone, component, create_effect, create_memo, create_selector, create_signal,
    view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Code, Heading, Icon, ListRow, Separator, TextInput,
    use_theme,
};
use block_editor_plugin::beui::{NodeId, Vec2};

use super::{format_commit_time, short_author};

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 16.0;
const INTRINSIC_WIDTH: f32 = 640.0;
const BRANCH_ROW_HEIGHT: f32 = 26.0;
const COMMIT_ROW_HEIGHT: f32 = 44.0;
const CHROME_HEIGHT: f32 = 192.0;

#[derive(Clone, PartialEq)]
struct BranchRow {
    name: String,
    head: CommitId,
    summary: String,
}

#[derive(Clone, PartialEq)]
struct CommitRow {
    id: CommitId,
    commit: Commit,
}

#[component]
pub fn RepositoryView(editor: Editor) -> NodeId {
    let data = editor.block::<VersionControlData>();
    let (selected, set_selected) = create_signal(MAIN_BRANCH.to_owned());
    let (draft, set_draft) = create_signal(String::new());
    let rows = data.project(|data| -> Vec<BranchRow> {
        data.branches()
            .iter()
            .map(|(name, head)| BranchRow {
                name: name.clone(),
                head: head.clone(),
                summary: data.commit(head).map_or_else(
                    || "unknown commit".to_owned(),
                    |commit| commit.message.clone(),
                ),
            })
            .collect()
    });
    let branches = create_memo(clone!(rows -> move || rows.get()));
    let histories = data.project(|data| -> Vec<(String, Vec<CommitRow>)> {
        data.branches()
            .iter()
            .map(|(name, head)| {
                let rows = data
                    .ancestors(head)
                    .into_iter()
                    .filter_map(|id| {
                        let commit = data.commit(&id).cloned()?;
                        Some(CommitRow { id, commit })
                    })
                    .collect();
                (name.clone(), rows)
            })
            .collect()
    });
    let history = create_memo(clone!(histories selected -> move || {
        let selected = selected.get();
        histories.with(|histories| {
            histories
                .iter()
                .find(|(name, _)| *name == selected)
                .map_or_else(Vec::new, |(_, rows)| rows.clone())
        })
    }));

    let read_only = editor.read_only();
    let chosen = create_selector(clone!(selected -> move || selected.get()));
    let head_of_selected = create_memo(clone!(branches selected -> move || {
        let selected = selected.get();
        branches.with(|branches| {
            branches
                .iter()
                .find(|branch| branch.name == selected)
                .map(|branch| branch.head.clone())
        })
    }));
    let cannot_create = create_memo(
        clone!(draft branches head_of_selected read_only -> move || {
            let name = draft.with(|draft| draft.trim().to_owned());
            read_only.get()
                || name.is_empty()
                || head_of_selected.get().is_none()
                || branches.with(|branches| branches.iter().any(|branch| branch.name == name))
        }),
    );
    let title = create_memo(clone!(selected -> move || format!("History  ({})", selected.get())));
    let no_history = create_memo(clone!(history -> move || history.with(Vec::is_empty)));

    let sized = editor.clone();
    create_effect(clone!(branches history -> move || {
        let height = CHROME_HEIGHT
            + BRANCH_ROW_HEIGHT * branches.with(Vec::len).max(1) as f32
            + COMMIT_ROW_HEIGHT * history.with(Vec::len).max(1) as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let created_draft = set_draft.clone();
    let create = clone!(data draft head_of_selected -> move || {
        let Some(commit) = head_of_selected.get_untracked() else {
            return;
        };
        let name = draft.with_untracked(|draft| draft.trim().to_owned());
        if name.is_empty() {
            return;
        }
        data.operate(VersionControlDataOperation::SetBranch {
            name,
            expected: None,
            commit,
        });
        created_draft.set(String::new());
    });
    let worktree_editor = editor.clone();
    let new_worktree = clone!(data -> move || {
        let Some(block) = data.handle().read() else {
            return;
        };
        let worktree = worktree_editor
            .client()
            .create_block(VersionControlWorktree::new(data.id(), &block));
        drop(block);
        worktree_editor
            .host()
            .open_block(worktree.id(), VersionControlWorktree::TYPE_ID);
    });

    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) focus_color={theme.accent.clone()}>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Heading content="Branches" />
                            <Branches branches={branches} chosen={chosen} select={set_selected} />
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <TextInput
                                    @sizing=ItemSize::Percent(100.0)
                                    value={draft}
                                    placeholder="New branch name"
                                    label="New branch name"
                                    @test_id={"repository.new-branch-name"}
                                    on_change={move |value| set_draft.set(value)}
                                />
                                <Button
                                    label="Create branch"
                                    variant=ButtonVariant::Primary
                                    disabled={cannot_create}
                                    @test_id={"repository.create-branch"}
                                    on_click={create}
                                />
                                <Button
                                    label="New worktree"
                                    variant=ButtonVariant::Secondary
                                    disabled={read_only}
                                    @test_id={"repository.new-worktree"}
                                    on_click={new_worktree}
                                />
                            </List>
                        </List>
                        <Separator />
                        <List spacing=6.0>
                            <Heading content={title} @test_id={"repository.history"} />
                            <Show condition={no_history}>
                                <Caption content="This branch has no commits yet." />
                            </Show>
                            <History history={history} />
                        </List>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn Branches(
    branches: Memo<Vec<BranchRow>>,
    chosen: Selector<String>,
    select: WriteSignal<String>,
) -> NodeId {
    let keys = create_memo(clone!(branches -> move || branches.with(|branches| {
        branches.iter().map(|branch| branch.name.clone()).collect::<Vec<String>>()
    })));
    view! {
        <List spacing=2.0>
            <ForEach keys={keys}>
                {move |name: String| {
                    let row = create_memo(clone!(branches name -> move || {
                        branches.with(|branches| {
                            branches.iter().find(|branch| branch.name == name).cloned()
                        })
                    }));
                    let selected = chosen.memo(name.clone());
                    let choose = clone!(select name -> move || select.set(name.clone()));
                    view! {
                        <BranchListRow
                            name={name}
                            row={row}
                            selected={selected}
                            on_click={choose}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn BranchListRow(
    name: String,
    row: Memo<Option<BranchRow>>,
    selected: Memo<bool>,
    on_click: block_editor_plugin::beui::reactive::ClickCallback,
) -> NodeId {
    let head = create_memo(clone!(row -> move || {
        row.with(|row| row.as_ref().map_or_else(String::new, |row| row.head.short()))
    }));
    let summary = create_memo(clone!(row -> move || {
        row.with(|row| row.as_ref().map_or_else(String::new, |row| row.summary.clone()))
    }));
    let theme = use_theme();
    view! {
        <ListRow
            selected={selected}
            @test_id={format!("repository.branch.{name}")}
            on_click={move || on_click.call()}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Icon glyph={ICON_ALT_ROUTE.to_owned()} color={theme.text_muted.clone()} />
                <Body content={name} />
                <Code content={head} />
                <Caption @sizing=ItemSize::Percent(100.0) content={summary} />
            </List>
        </ListRow>
    }
}

#[component]
fn History(history: Memo<Vec<CommitRow>>) -> NodeId {
    let keys = create_memo(clone!(history -> move || {
        (0..history.with(Vec::len)).collect::<Vec<usize>>()
    }));
    view! {
        <List spacing=10.0>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let row = create_memo(clone!(history -> move || {
                        history.with(|history| history.get(index).cloned())
                    }));
                    view! {
                        <CommitEntry row={row} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn CommitEntry(row: Memo<Option<CommitRow>>) -> NodeId {
    let message = create_memo(clone!(row -> move || {
        row.with(|row| row.as_ref().map_or_else(String::new, |row| row.commit.message.clone()))
    }));
    let id = create_memo(clone!(row -> move || {
        row.with(|row| row.as_ref().map_or_else(String::new, |row| row.id.short()))
    }));
    let author = create_memo(clone!(row -> move || {
        row.with(|row| {
            row.as_ref()
                .map_or_else(String::new, |row| short_author(row.commit.author))
        })
    }));
    let time = create_memo(clone!(row -> move || {
        row.with(|row| {
            row.as_ref()
                .map_or_else(String::new, |row| format_commit_time(row.commit.time))
        })
    }));
    let theme = use_theme();
    let muted = theme.text_muted.clone();
    let author_icon = theme.text_muted.clone();
    let time_icon = theme.text_muted.clone();
    view! {
        <List spacing=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Icon glyph={ICON_COMMIT.to_owned()} color={muted} />
                <Body content={message} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <Code content={id} />
                <Icon glyph={ICON_PERSON.to_owned()} color={author_icon} />
                <Caption content={author} />
                <Icon glyph={ICON_SCHEDULE.to_owned()} color={time_icon} />
                <Caption content={time} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Separator />
        </List>
    }
}
