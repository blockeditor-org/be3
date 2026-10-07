use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::{EditorViewContent, WORKSPACE_EDITOR};
use block_editor_beui::beui::reactive::{Memo, create_memo};
use block_editor_beui::root_settings::{RootSettings, SettingsGraph};
use block_shell::Workspace;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Session {
    pub(crate) profile: Uuid,
    pub(crate) name: String,
}

pub(crate) fn sessions(workspace: &Rc<Workspace>) -> Memo<Vec<Session>> {
    let workspace = Rc::clone(workspace);
    let settings = RefCell::new(RootSettings::default());
    create_memo(move || {
        let editor = workspace.editor();
        let Some(block) = settings.borrow_mut().find(editor) else {
            return Vec::new();
        };
        let Some(held) = editor.settings(block) else {
            return Vec::new();
        };
        let mut sessions: Vec<Session> = held
            .profiles()
            .into_iter()
            .filter(|profile| {
                editor
                    .content_of::<EditorViewContent>(*profile)
                    .read(|view| view.root().editor)
                    == Some(WORKSPACE_EDITOR)
            })
            .map(|profile| Session {
                profile,
                name: workspace
                    .info(profile)
                    .and_then(|info| info.name)
                    .unwrap_or_else(|| "Session".to_owned()),
            })
            .collect();
        sessions.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.profile.cmp(&right.profile))
        });
        sessions
    })
}
