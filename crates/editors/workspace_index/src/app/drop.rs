use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::{Memo, create_memo, create_signal};

use super::entries::Folder;

pub(crate) fn watch(editor: &Editor, folder: &Rc<Folder>) -> Memo<Option<bool>> {
    let (state, set_state) = create_signal(None::<bool>);
    let drag = editor.drag();
    let folder = Rc::clone(folder);
    let here = editor.clone();
    editor.each_frame(move || {
        let Some(drag) = drag.get_untracked() else {
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
    });
    create_memo(move || state.get())
}
