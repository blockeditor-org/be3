use block_editor_beui::be_block::Root as _;
use block_editor_beui::be_block::{CheckoutContent, ConflictKind, Repository};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    BlockLink, ChildTarget, ConflictSide, Editor, VersionChangeKind, VersionCommand,
    VersionHistory, short_id,
};
use uuid::Uuid;

const PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

type ChangeRow = (Uuid, Uuid, Option<String>, &'static str);

type ConflictRow = (
    Uuid,
    Uuid,
    &'static str,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);

fn change_label(kind: VersionChangeKind) -> &'static str {
    match kind {
        VersionChangeKind::Added => "Added",
        VersionChangeKind::Removed => "Removed",
        VersionChangeKind::Modified => "Changed",
        VersionChangeKind::Moved => "Moved or renamed",
    }
}

fn conflict_label(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::Content => "Both sides changed it",
        ConflictKind::Placement => "Both sides moved it",
        ConflictKind::Name => "Both sides renamed it",
        ConflictKind::EditedAndRemoved => "Changed on one side and removed on the other",
        ConflictKind::Metadata => "Both sides changed its details",
    }
}

#[component]
pub fn CheckoutView(editor: Editor) -> NodeId {
    let content = editor.block_content::<CheckoutContent>();
    let state = content.project(|content| content.root());
    let status = editor.version_status();
    let editable = editor.editable();
    let summary = create_memo(move || {
        state.with(|state| match state.base {
            Some(base) => format!(
                "On {}, at {}",
                state.branch,
                short_id(base.hash().as_bytes())
            ),
            None => format!("On {}, with nothing committed yet", state.branch),
        })
    });
    let repository = create_memo(move || {
        state.with(|state| {
            state
                .repository
                .map(|repository| ChildTarget::new(repository, Repository::CONTENT_TYPE))
        })
    });
    let idle = create_memo(move || editable.get() && !status.with(|status| status.busy));
    let blocked = create_memo(move || !idle.get());
    let note = create_memo(move || {
        status.with(|status| match (status.busy, &status.error) {
            (true, _) => "Working…".to_owned(),
            (false, Some(error)) => error.clone(),
            (false, None) => String::new(),
        })
    });
    let has_note = create_memo(move || !note.with(String::is_empty));
    let behind = create_memo(move || status.with(|status| status.behind));
    let changes = create_memo(move || {
        status.with(|status| {
            status
                .changes
                .iter()
                .map(|change| -> ChangeRow {
                    (
                        Uuid::from_bytes(change.block_id),
                        Uuid::from_bytes(change.block_type),
                        change.name.clone(),
                        change_label(change.kind),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let unchanged = create_memo(move || changes.with(Vec::is_empty));
    let conflicts = create_memo(move || {
        state.with(|state| {
            state
                .conflicts
                .iter()
                .map(|conflict| -> ConflictRow {
                    (
                        conflict.block,
                        conflict.content_type,
                        conflict_label(conflict.kind),
                        conflict.base,
                        conflict.ours,
                        conflict.theirs,
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let conflicted = create_memo(move || !conflicts.with(Vec::is_empty));
    let branches = create_memo(move || {
        let current = state.with(|state| state.branch.clone());
        status.with(|status| {
            status
                .branches
                .iter()
                .map(|branch| (branch.name.clone(), branch.name == current))
                .collect::<Vec<_>>()
        })
    });
    let log = create_memo(move || status.with(|status| status.log.clone()));

    let message = create_signal(String::new());
    let commit_blocked = create_memo(move || {
        blocked.get()
            || unchanged.get()
            || conflicted.get()
            || message.with(|message| message.trim().is_empty())
    });
    let branch_name = create_signal(String::new());
    let branch_blocked =
        create_memo(move || blocked.get() || branch_name.with(|name| name.trim().is_empty()));

    let change_rows = move |row: ChangeRow| {
        view! {
            <ChangeLine editor row />
        }
    };
    let conflict_rows = move |row: ConflictRow| {
        let blocked = blocked;
        view! {
            <ConflictLine editor blocked row />
        }
    };
    let branch_rows = move |(name, current): (String, bool)| {
        let blocked = blocked;
        view! {
            <BranchLine editor blocked name current />
        }
    };

    view! {
        <Frame fill=ColorRole::Background padding=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Text style=TextStyle::HEADING>"Checkout"</Text>
                            <Text style=TextStyle::CAPTION @test_id={"checkout.summary"}>
                                {summary}
                            </Text>
                            <BlockLink
                                editor={editor}
                                block={repository}
                                @test_id={"checkout.repository"}
                            />
                            <Show condition={has_note}>
                                <Text style=TextStyle::CAPTION wrap=true @test_id={"checkout.note"}>
                                    {note}
                                </Text>
                            </Show>
                        </List>
                        <Show condition={behind}>
                            <Card>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <Text @sizing=ItemSize::Percent(100.0)>
                                        "The branch has moved on since this checkout's base."
                                    </Text>
                                    <Button
                                        label="Bring changes in"
                                        variant=ButtonVariant::Primary
                                        disabled={blocked}
                                        @test_id={"checkout.update"}
                                        on_click={move || editor.version(VersionCommand::Update)}
                                    />
                                </List>
                            </Card>
                        </Show>
                        <Show condition={conflicted}>
                            <Card>
                                <List spacing=12.0>
                                    <Text style=TextStyle::HEADING>"Conflicts"</Text>
                                    <ForEach keys={conflicts} view={conflict_rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"Changes"</Text>
                                <Show condition={unchanged}>
                                    <Text style=TextStyle::CAPTION>
                                        "Nothing has changed since the last commit."
                                    </Text>
                                </Show>
                                <ForEach keys={changes} view={change_rows} />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <TextInput
                                        @sizing=ItemSize::Percent(100.0)
                                        value={message}
                                        placeholder="What changed?"
                                        label="Commit message"
                                        @test_id={"checkout.message"}
                                    />
                                    <Button
                                        label="Commit"
                                        variant=ButtonVariant::Primary
                                        disabled={commit_blocked}
                                        @test_id={"checkout.commit"}
                                        on_click={move || {
                                            let text = message.get().trim().to_owned();
                                            editor.version(VersionCommand::Commit { message: text });
                                            message.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"Branches"</Text>
                                <ForEach keys={branches} view={branch_rows} />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <TextInput
                                        @sizing=ItemSize::Percent(100.0)
                                        value={branch_name}
                                        placeholder="New branch"
                                        label="New branch name"
                                        @test_id={"checkout.branch-name"}
                                    />
                                    <Button
                                        label="Make branch"
                                        variant=ButtonVariant::Secondary
                                        disabled={branch_blocked}
                                        @test_id={"checkout.make-branch"}
                                        on_click={move || {
                                            let name = branch_name.get().trim().to_owned();
                                            editor.version(VersionCommand::CreateBranch { name });
                                            branch_name.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"History"</Text>
                                <VersionHistory log={log} />
                            </List>
                        </Card>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn ChangeLine(editor: Editor, row: ChangeRow) -> NodeId {
    let (block, block_type, name, label) = row;
    let removed = label == change_label(VersionChangeKind::Removed);
    let name = name.unwrap_or_else(|| "Untitled".to_owned());
    let target = Some(ChildTarget::new(block, block_type));
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id={format!("checkout.change.{block}")}
        >
            <Text style=TextStyle::CAPTION>{label}</Text>
            <Show condition={removed}>|| {
                let name = name.clone(); view! {
                    <Text>{name}</Text>
                }
            }</Show>
            <Show condition={!removed}>
                <BlockLink editor={editor} block={target} />
            </Show>
        </List>
    }
}

#[component]
fn ConflictLine(editor: Editor, blocked: Memo<bool>, row: ConflictRow) -> NodeId {
    let (block, block_type, label, base, ours, theirs) = row;
    let side = |side: Option<Uuid>| side.map(|side| ChildTarget::new(side, block_type));
    let choose = |take: ConflictSide| {
        move || {
            editor.version(VersionCommand::Resolve {
                block_id: block.into_bytes(),
                take,
            });
        }
    };
    let compared = base.is_some() || ours.is_some() || theirs.is_some();
    let (take_base, take_ours, take_theirs) = (
        choose(ConflictSide::Base),
        choose(ConflictSide::Ours),
        choose(ConflictSide::Theirs),
    );
    let (base, ours, theirs) = (side(base), side(ours), side(theirs));
    let keep = choose(ConflictSide::Merged);
    let working = Some(ChildTarget::new(block, block_type));
    view! {
        <List spacing=8.0 @test_id={format!("checkout.conflict.{block}")}>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <BlockLink editor={editor} block={working} />
                <Text style=TextStyle::CAPTION>{label}</Text>
            </List>
            <List spacing=8.0>
                <Show condition={compared}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <Text style=TextStyle::CAPTION>"Base"</Text>
                        <BlockLink editor={editor} block={base} fallback="none" />
                        <Text style=TextStyle::CAPTION>"Yours"</Text>
                        <BlockLink editor={editor} block={ours} fallback="none" />
                        <Text style=TextStyle::CAPTION>"Theirs"</Text>
                        <BlockLink editor={editor} block={theirs} fallback="none" />
                    </List>
                </Show>
                <Show condition={compared}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            label="Take base"
                            variant=ButtonVariant::Secondary
                            disabled={blocked}
                            @test_id={format!("checkout.base.{block}")}
                            on_click={take_base}
                        />
                        <Button
                            label="Take yours"
                            variant=ButtonVariant::Secondary
                            disabled={blocked}
                            @test_id={format!("checkout.ours.{block}")}
                            on_click={take_ours}
                        />
                        <Button
                            label="Take theirs"
                            variant=ButtonVariant::Secondary
                            disabled={blocked}
                            @test_id={format!("checkout.theirs.{block}")}
                            on_click={take_theirs}
                        />
                    </List>
                </Show>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Keep as merged"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={format!("checkout.keep.{block}")}
                    on_click={keep}
                />
            </List>
        </List>
    }
}

#[component]
fn BranchLine(editor: Editor, blocked: Memo<bool>, name: String, current: bool) -> NodeId {
    let branch = name.clone();
    let disabled = create_memo(move || blocked.get() || current);
    let label = match current {
        true => "Current",
        false => "Switch",
    };
    let branch_test_id = format!("checkout.branch.{name}");
    let switch_test_id = format!("checkout.switch.{name}");
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id=branch_test_id
        >
            <Text @sizing=ItemSize::Percent(100.0)>{name}</Text>
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={disabled}
                @test_id=switch_test_id
                on_click={move || editor.version(VersionCommand::Switch { branch: branch.clone() })}
            />
        </List>
    }
}
