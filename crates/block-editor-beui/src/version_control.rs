use beui::NodeId;
use beui::icons::ICON_UNDO_2;
use beui::reactive::*;
use beui::styled::*;
use block_editor_plugin::{VersionCommand, VersionCommit, VersionOperation, short_id};

use crate::Editor;
use beui::{Align, Direction, ItemSize};

#[component]
pub fn VersionHistory(log: Memo<Vec<VersionCommit>>) -> NodeId {
    let rows = create_memo(move || {
        log.with(|log| {
            log.iter()
                .map(|commit| {
                    (
                        commit.id,
                        commit.change,
                        commit.message.clone(),
                        commit.conflicted,
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let empty = create_memo(move || rows.with(Vec::is_empty));
    view! {
        <List spacing=Space::Sm>
            <beui::reactive::Show condition={empty}>
                <Text style=TextStyle::CAPTION>"No changes have been saved yet."</Text>
            </beui::reactive::Show>
            <ForEach
                keys={rows}
            >|(id, change, message, conflicted): ([u8; 32], [u8; 16], String, bool)| {
                let commit_test_id = format!("version.commit.{}", short_id(&id));
                let message = match message.is_empty() {
                    true => "(no description)".to_owned(),
                    false => message,
                };
                view! {
                    <List
                        direction=Direction::Horizontal
                        spacing=Space::Md
                        @test_id={commit_test_id}
                    >
                        <Text style=TextStyle::CAPTION>{short_id(&change)}</Text>
                        <Text>{message}</Text>
                        <beui::reactive::Show condition={conflicted}>
                            <Text style=TextStyle::CAPTION>"still has clashing edits"</Text>
                        </beui::reactive::Show>
                    </List>
                }
            }</ForEach>
        </List>
    }
}

#[component]
pub fn VersionSteps(
    editor: Editor,
    operations: Memo<Vec<VersionOperation>>,
    blocked: Memo<bool>,
) -> NodeId {
    let rows = create_memo(move || {
        operations.with(|operations| {
            operations
                .iter()
                .map(|operation| {
                    (
                        operation.id,
                        operation.description.clone(),
                        operation.undone,
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let empty = create_memo(move || rows.with(Vec::is_empty));
    view! {
        <List spacing=Space::Sm>
            <beui::reactive::Show condition={empty}>
                <Text style=TextStyle::CAPTION>"Nothing has been done yet."</Text>
            </beui::reactive::Show>
            <ForEach keys={rows}>|(id, description, undone): ([u8; 16], String, bool)| {
                let short = short_id(&id);
                let (step_test_id, undo_test_id) =
                    (format!("version.step.{short}"), format!("version.undo.{short}"));
                let disabled = create_memo(move || blocked.get() || undone);
                view! {
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=Space::Md
                        @test_id=step_test_id
                    >
                        <Text @sizing=ItemSize::Percent(100.0)>{description}</Text>
                        <beui::reactive::Show condition={undone}>
                            <Text style=TextStyle::CAPTION>"Undone"</Text>
                        </beui::reactive::Show>
                        <Button
                            label="Undo"
                            glyph=ICON_UNDO_2
                            variant=ButtonVariant::Ghost
                            size=ButtonSize::Compact
                            disabled={disabled}
                            @test_id=undo_test_id
                            on_click={move || editor.version(VersionCommand::Undo { operation: id })}
                        />
                    </List>
                }
            }</ForEach>
        </List>
    }
}
