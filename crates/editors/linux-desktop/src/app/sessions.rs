use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::BlockParent;
use block_editor_beui::be_block::{
    EditorView, EditorViewContent, Settings, SettingsContent, WORKSPACE_EDITOR,
};
use block_editor_beui::beui::reactive::{Memo, clone, create_memo};
use block_editor_beui::root_settings::{RootSettings, SettingsGraph};
use block_shell::Workspace;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Session {
    pub(crate) profile: Uuid,
    pub(crate) name: String,
}

#[derive(Clone)]
pub(crate) struct Sessions {
    workspace: Rc<Workspace>,
    pub(crate) settings: Memo<Option<Uuid>>,
    pub(crate) listed: Memo<Vec<Session>>,
}

impl Sessions {
    pub(crate) fn new(workspace: &Rc<Workspace>) -> Self {
        let found = RefCell::new(RootSettings::default());
        let finding = Rc::clone(workspace);
        let settings = create_memo(move || found.borrow_mut().find(finding.editor()));
        let listing = Rc::clone(workspace);
        let listed = create_memo(clone!(settings -> move || listed(&listing, settings.get())));
        Self {
            workspace: Rc::clone(workspace),
            settings,
            listed,
        }
    }

    pub(crate) fn open(&self, profile: Uuid) {
        self.workspace.open_block(profile, WORKSPACE_EDITOR);
    }

    pub(crate) fn create(&self) {
        let Some(settings) = self.settings.get_untracked() else {
            return;
        };
        let editor = self.workspace.editor();
        let count = self.listed.with_untracked(Vec::len);
        let profile = editor.blocks().create_named(
            &EditorView::document(WORKSPACE_EDITOR, None),
            BlockParent::Block(settings),
            format!("Session {}", count + 1),
        );
        editor
            .content_of::<SettingsContent>(settings)
            .operate(Settings::list_profile(profile));
        self.open(profile);
    }
}

fn listed(workspace: &Workspace, settings: Option<Uuid>) -> Vec<Session> {
    let editor = workspace.editor();
    let Some(held) = settings.and_then(|block| editor.settings(block)) else {
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
}
