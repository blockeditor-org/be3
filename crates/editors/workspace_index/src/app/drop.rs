use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    Memo, WriteSignal, create_effect, create_memo, create_signal, untrack,
};
use block_editor_beui::{Drag, Editor};

use super::entries::Folder;

pub(crate) fn watch(editor: &Editor, folder: &Rc<Folder>) -> Memo<Option<bool>> {
    let (state, set_state) = create_signal(None::<bool>);
    let drag = editor.drag();
    let folder = Rc::clone(folder);
    let here = editor.clone();
    create_effect(move || {
        let drag = drag.get();
        untrack(|| land(&here, &folder, &set_state, drag));
    });
    create_memo(move || state.get())
}

fn land(here: &Editor, folder: &Folder, set_state: &WriteSignal<Option<bool>>, drag: Option<Drag>) {
    let Some(drag) = drag else {
        set_state.set(None);
        return;
    };
    if !here.content_rect().contains(drag.position) {
        set_state.set(None);
        return;
    }
    let welcome = folder.accepts(drag.block_id, here.block_id());
    if drag.dropped {
        set_state.set(None);
        if welcome {
            folder.add(drag.block_id);
        }
        return;
    }
    here.accept_drag(welcome);
    set_state.set(Some(welcome));
}
