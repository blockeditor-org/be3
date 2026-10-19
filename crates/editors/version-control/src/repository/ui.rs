use block_editor_beui::be_block::Root as _;
use block_editor_beui::be_block::{Repository, RepositoryContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    BlockFilter, BlockLink, ChildTarget, Editor, VersionCommand, VersionHistory, short_id,
};

const PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

#[component]
pub fn RepositoryView(editor: Editor) -> NodeId {
    let content = editor.block_content::<RepositoryContent>();
    let upstream = content.project(|content| content.root().upstream);
    let status = editor.version_status();
    let editable = editor.editable();
    let branches = create_memo(move || {
        status.with(|status| {
            status
                .branches
                .iter()
                .map(|branch| (branch.name.clone(), short_id(&branch.head)))
                .collect::<Vec<_>>()
        })
    });
    let log = create_memo(move || status.with(|status| status.log.clone()));
    let unversioned = create_memo(move || branches.with(Vec::is_empty));
    let versioned = create_memo(move || !unversioned.get());
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
    let upstream_target = create_memo(move || {
        upstream
            .get()
            .map(|upstream| ChildTarget::new(upstream, Repository::CONTENT_TYPE))
    });
    let has_upstream = create_memo(move || upstream.get().is_some());
    let no_upstream = create_memo(move || !has_upstream.get());
    let pull_blocked = create_memo(move || blocked.get() || no_upstream.get());

    let choose = move || {
        editor.pick_block(
            BlockFilter {
                name: "Choose what to version".into(),
                ..BlockFilter::default()
            },
            move |picked| {
                if let Ok(picked) = picked {
                    editor.version(VersionCommand::Adopt {
                        block_id: picked.id.into_bytes(),
                    });
                }
            },
        );
    };
    let adopt_blocked = blocked;
    let fork_blocked = blocked;
    let push_blocked = pull_blocked;
    let rows = move |(name, head): (String, String)| {
        let blocked = blocked;
        view! {
            <BranchRow editor blocked name head />
        }
    };

    view! {
        <Frame fill=ColorRole::Background padding=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Text style=TextStyle::HEADING string="Repository" />
                            <Show condition={has_note}>
                                <Text
                                    style=TextStyle::CAPTION
                                    string={note}
                                    wrap=true
                                    @test_id={"repository.note"}
                                />
                            </Show>
                        </List>
                        <Show condition={unversioned}>
                            <Card>
                                <List spacing=10.0>
                                    <Text string="Nothing is versioned here yet." />
                                    <Text
                                        style=TextStyle::CAPTION
                                        string="Choose a block and everything below it becomes this repository's first commit, with a checkout around it to edit it in."
                                        wrap=true
                                    />
                                    <Button
                                        label="Choose what to version"
                                        variant=ButtonVariant::Primary
                                        disabled={adopt_blocked}
                                        @test_id={"repository.adopt"}
                                        on_click={choose}
                                    />
                                </List>
                            </Card>
                        </Show>
                        <Show condition={versioned}>
                            <Card>
                                <List spacing=10.0>
                                    <Text style=TextStyle::HEADING string="Branches" />
                                    <ForEach keys={branches} view={rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING string="Upstream" />
                                <Show condition={has_upstream}>
                                    <BlockLink
                                        editor={editor}
                                        block={upstream_target}
                                        @test_id={"repository.upstream"}
                                    />
                                </Show>
                                <Show condition={no_upstream}>
                                    <Text
                                        style=TextStyle::CAPTION
                                        string="A fork follows the repository it was made from, which it can pull from and push to."
                                        wrap=true
                                    />
                                </Show>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <Button
                                        label="Fork"
                                        variant=ButtonVariant::Secondary
                                        disabled={fork_blocked}
                                        @test_id={"repository.fork"}
                                        on_click={move || editor.version(VersionCommand::Fork)}
                                    />
                                    <Button
                                        label="Pull"
                                        variant=ButtonVariant::Secondary
                                        disabled={pull_blocked}
                                        @test_id={"repository.pull"}
                                        on_click={move || editor.version(VersionCommand::PullUpstream)}
                                    />
                                    <Button
                                        label="Push"
                                        variant=ButtonVariant::Secondary
                                        disabled={push_blocked}
                                        @test_id={"repository.push"}
                                        on_click={move || editor.version(VersionCommand::PushUpstream)}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING string="History" />
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
fn BranchRow(editor: Editor, blocked: Memo<bool>, name: String, head: String) -> NodeId {
    let branch = name.clone();
    let branch_test_id = format!("repository.branch.{name}");
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id=branch_test_id
        >
            <Text @sizing=ItemSize::Percent(100.0) string={name.clone()} />
            <Text style=TextStyle::CAPTION string={head} />
            <Button
                label="Check out"
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={format!("repository.checkout.{name}")}
                on_click={move || editor.version(VersionCommand::NewCheckout { branch: branch.clone() })}
            />
        </List>
    }
}
