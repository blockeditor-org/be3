use beui::Overflow;
use beui::{Align, Direction, ItemSize};
use block_editor_beui::be_block::Root as _;
use block_editor_beui::be_block::{Repository, RepositoryContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    BlockFilter, BlockLink, ChildTarget, Editor, VersionCommand, VersionHistory, VersionSteps,
    short_id,
};

const PADDING: Space = Space::Xl3;
const SECTION_SPACING: Space = Space::Xl2;

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
    let operations = create_memo(move || status.with(|status| status.operations.clone()));
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

    let adopt = NodeRef::new();
    let choose = move || {
        editor.pick_block(
            BlockFilter {
                name: "Choose what to version".into(),
                ..BlockFilter::default()
            },
            &adopt,
            move |picked| {
                if let Ok(picked) = picked {
                    editor.version(VersionCommand::Adopt {
                        block_id: picked.id().into_bytes(),
                    });
                }
            },
        );
    };
    let rows = move |(name, head): (String, String)| {
        view! {
            <BranchRow editor blocked name head />
        }
    };

    view! {
        <List spacing=Space::None>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=PADDING>
                    <List spacing=SECTION_SPACING>
                        <List spacing=Space::Sm>
                            <Text style=TextStyle::HEADING>"Repository"</Text>
                            <Show condition={has_note}>
                                <Text style=TextStyle::CAPTION @test_id={"repository.note"}>
                                    {note}
                                </Text>
                            </Show>
                        </List>
                        <Show condition={unversioned}>
                            <Card>
                                <List spacing=Space::Md>
                                    <Text>"Nothing is versioned here yet."</Text>
                                    <Text style=TextStyle::CAPTION>
                                        "Choose a block, and everything inside it is saved as the first change. From then on its edits gather into changes you describe and save, and any step can be undone."
                                    </Text>
                                    <Button
                                        @node_ref={&adopt}
                                        label="Choose what to version"
                                        variant=ButtonVariant::Primary
                                        disabled={blocked}
                                        @test_id={"repository.adopt"}
                                        on_click={choose}
                                    />
                                </List>
                            </Card>
                        </Show>
                        <Show condition={versioned}>
                            <Card>
                                <List spacing=Space::Md>
                                    <Text style=TextStyle::HEADING>"Branches"</Text>
                                    <ForEach keys={branches} view={rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=Space::Md>
                                <Text style=TextStyle::HEADING>"Original"</Text>
                                <Show condition={has_upstream}>
                                    <BlockLink
                                        editor={editor}
                                        block={upstream_target}
                                        @test_id={"repository.upstream"}
                                    />
                                </Show>
                                <Show condition={no_upstream}>
                                    <Text style=TextStyle::CAPTION>
                                        "A copy of a repository follows the one it was made from: it can bring in its changes and send its own back."
                                    </Text>
                                </Show>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Md
                                >
                                    <Button
                                        label="Copy this history"
                                        variant=ButtonVariant::Secondary
                                        disabled={blocked}
                                        @test_id={"repository.fork"}
                                        on_click={move || editor.version(VersionCommand::Fork)}
                                    />
                                    <Button
                                        label="Bring in"
                                        variant=ButtonVariant::Secondary
                                        disabled={pull_blocked}
                                        @test_id={"repository.pull"}
                                        on_click={move || editor.version(VersionCommand::PullUpstream)}
                                    />
                                    <Button
                                        label="Send back"
                                        variant=ButtonVariant::Secondary
                                        disabled={pull_blocked}
                                        @test_id={"repository.push"}
                                        on_click={move || editor.version(VersionCommand::PushUpstream)}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=Space::Md>
                                <Text style=TextStyle::HEADING>"Saved changes"</Text>
                                <VersionHistory log={log} />
                            </List>
                        </Card>
                        <Card>
                            <List spacing=Space::Md>
                                <Text style=TextStyle::HEADING>"What was done"</Text>
                                <VersionSteps
                                    editor={editor}
                                    operations={operations}
                                    blocked={blocked}
                                />
                            </List>
                        </Card>
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[component]
fn BranchRow(editor: Editor, blocked: Memo<bool>, name: String, head: String) -> NodeId {
    let branch = name.clone();
    let branch_test_id = format!("repository.branch.{name}");
    let checkout_test_id = format!("repository.checkout.{name}");
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=Space::Md
            @test_id=branch_test_id
        >
            <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>{name}</Text>
            <Text style=TextStyle::CAPTION>{head}</Text>
            <Button
                label="Open a copy to edit"
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id=checkout_test_id
                on_click={move || editor.version(VersionCommand::NewCheckout { branch: branch.clone() })}
            />
        </List>
    }
}
