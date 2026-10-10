use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{clone, component, create_memo, view};
use block_editor_beui::beui::styled::{KEEP_CHANGES_TIMEOUT, KeepChanges};
use block_editor_beui::{DisplayAnswer, DisplayConfirmation, Editor};

#[component]
pub(super) fn DisplayPrompt(editor: Editor) -> NodeId {
    let pending = editor.host_value::<DisplayConfirmation>();
    let open = create_memo(clone!(pending -> move || pending.get().is_some()));
    let round = create_memo(clone!(pending -> move || {
        pending.get().map_or(0, |pending| pending.round)
    }));
    let timeout = create_memo(clone!(pending -> move || {
        pending
            .get()
            .map_or(KEEP_CHANGES_TIMEOUT, |pending| pending.timeout)
    }));
    let answering = round.clone();
    let keeping = editor.clone();
    view! {
        <KeepChanges
            open
            title="Keep these display settings?"
            timeout
            round={round.clone()}
            id="display.keep"
            on_keep={move || keeping.act(DisplayAnswer::Keep(answering.get_untracked()))}
            on_revert={move || editor.act(DisplayAnswer::Revert(round.get_untracked()))}
        />
    }
}
