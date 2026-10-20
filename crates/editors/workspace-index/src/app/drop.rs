use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Drag, Editor};

use super::entries::Folder;

pub(crate) fn watch(editor: Editor, folder: Folder) -> Memo<Option<bool>> {
    let state = create_signal(None::<bool>);
    let drag = editor.drag();
    create_effect(move || {
        let drag = drag.get();
        untrack(|| land(editor, &folder, state, drag));
    });
    create_memo(move || state.get())
}

fn land(editor: Editor, folder: &Folder, state: Signal<Option<bool>>, drag: Option<Drag>) {
    let Some(drag) = drag else {
        state.set(None);
        return;
    };
    if !editor.content_rect().contains(drag.position) {
        state.set(None);
        return;
    }
    let welcome = folder.accepts(drag.block_id, editor.block_id());
    if drag.dropped {
        state.set(None);
        if welcome {
            folder.add(drag.block_id);
        }
        return;
    }
    editor.accept_drag(welcome);
    state.set(Some(welcome));
}
