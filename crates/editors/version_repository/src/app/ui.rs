use block_editor_plugin::be_block::Root as _;
use block_editor_plugin::be_block::{Repository, RepositoryContent};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component, create_memo,
    view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Heading, Scroll, use_theme,
};
use block_editor_plugin::{
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
    let branches = create_memo(clone!(status -> move || {
        status.with(|status| {
            status
                .branches
                .iter()
                .map(|branch| (branch.name.clone(), short_id(&branch.head)))
                .collect::<Vec<_>>()
        })
    }));
    let log = create_memo(clone!(status -> move || status.with(|status| status.log.clone())));
    let unversioned = create_memo(clone!(branches -> move || branches.with(Vec::is_empty)));
    let versioned = create_memo(clone!(unversioned -> move || !unversioned.get()));
    let idle = create_memo(clone!(status editable -> move || {
        editable.get() && !status.with(|status| status.busy)
    }));
    let blocked = create_memo(clone!(idle -> move || !idle.get()));
    let note = create_memo(clone!(status -> move || {
        status.with(|status| match (status.busy, &status.error) {
            (true, _) => "Working…".to_owned(),
            (false, Some(error)) => error.clone(),
            (false, None) => String::new(),
        })
    }));
    let has_note = create_memo(clone!(note -> move || !note.with(String::is_empty)));
    let upstream_target = create_memo(clone!(upstream -> move || {
        upstream
            .get()
            .map(|upstream| ChildTarget::new(upstream, Repository::CONTENT_TYPE))
    }));
    let has_upstream = create_memo(clone!(upstream -> move || upstream.get().is_some()));
    let no_upstream = create_memo(clone!(has_upstream -> move || !has_upstream.get()));
    let pull_blocked =
        create_memo(clone!(blocked no_upstream -> move || blocked.get() || no_upstream.get()));

    let choosing = editor.clone();
    let choose = move || {
        let adopting = choosing.clone();
        choosing.pick_block(
            BlockFilter {
                name: "Choose what to version".into(),
                ..BlockFilter::default()
            },
            move |picked| {
                if let Ok(picked) = picked {
                    adopting.version(VersionCommand::Adopt {
                        block_id: picked.id.into_bytes(),
                    });
                }
            },
        );
    };
    let adopt_blocked = blocked.clone();
    let fork_blocked = blocked.clone();
    let push_blocked = pull_blocked.clone();
    let upstream_editor = editor.clone();
    let forking = editor.clone();
    let pulling = editor.clone();
    let pushing = editor.clone();
    let checking_out = editor.clone();
    let rows = clone!(blocked -> move |(name, head): (String, String)| {
        let editor = checking_out.clone();
        let blocked = blocked.clone();
        view! {
            <BranchRow editor blocked name head />
        }
    });

    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Heading content="Repository" />
                            <Show condition={has_note}>
                                <Caption content={note} wrap=true @test_id={"repository.note"} />
                            </Show>
                        </List>
                        <Show condition={unversioned}>
                            <Card>
                                <List spacing=10.0>
                                    <Body content="Nothing is versioned here yet." />
                                    <Caption
                                        content="Choose a block and everything below it becomes this repository's first commit, with a checkout around it to edit it in."
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
                                    <Heading content="Branches" />
                                    <ForEach keys={branches} view={rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=10.0>
                                <Heading content="Upstream" />
                                <Show condition={has_upstream}>
                                    <BlockLink
                                        editor={upstream_editor}
                                        block={upstream_target}
                                        @test_id={"repository.upstream"}
                                    />
                                </Show>
                                <Show condition={no_upstream}>
                                    <Caption
                                        content="A fork follows the repository it was made from, which it can pull from and push to."
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
                                        on_click={move || forking.version(VersionCommand::Fork)}
                                    />
                                    <Button
                                        label="Pull"
                                        variant=ButtonVariant::Secondary
                                        disabled={pull_blocked}
                                        @test_id={"repository.pull"}
                                        on_click={move || pulling.version(VersionCommand::PullUpstream)}
                                    />
                                    <Button
                                        label="Push"
                                        variant=ButtonVariant::Secondary
                                        disabled={push_blocked}
                                        @test_id={"repository.push"}
                                        on_click={move || pushing.version(VersionCommand::PushUpstream)}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Heading content="History" />
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
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id={format!("repository.branch.{name}")}
        >
            <Body @sizing=ItemSize::Percent(100.0) content={name.clone()} />
            <Caption content={head} />
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
