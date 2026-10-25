use block_editor_beui::BlockParent;
use block_editor_beui::be_block::BlockMetadata;
use block_editor_beui::be_block::{
    BlockContent, Settings, SettingsContent, VIEW_EDITOR, WORKSPACE_EDITOR,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::root_settings::RootSettings;
use block_shell::Workspace;
use uuid::Uuid;
use workspace_ui_types::EditorViewContent;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Session {
    pub(crate) block: Uuid,
    pub(crate) name: String,
}

#[derive(Clone, Copy)]
pub(crate) struct Sessions {
    workspace: Workspace,
    pub(crate) settings: Memo<Option<Uuid>>,
    pub(crate) listed: Memo<Vec<Session>>,
}

impl Sessions {
    pub(crate) fn new(workspace: Workspace) -> Self {
        let found = RootSettings::default();
        let settings = create_memo(move || found.find(&workspace.editor()));
        let listed = create_memo(move || listed(&workspace));
        Self {
            workspace,
            settings,
            listed,
        }
    }

    pub(crate) fn open(&self, session: Uuid) {
        self.workspace
            .open_block(session, EditorViewContent::CONTENT_TYPE);
    }

    pub(crate) fn create(&self) {
        let Some(settings) = self.settings.get_untracked() else {
            return;
        };
        let editor = self.workspace.editor();
        let count = self.listed.with_untracked(Vec::len);
        let session = editor.blocks().create_with(
            EditorViewContent::CONTENT_TYPE,
            None,
            BlockParent::Block(settings),
            BlockMetadata::view(WORKSPACE_EDITOR, Some(format!("Session {}", count + 1))),
        );
        editor
            .content_of::<SettingsContent>(settings)
            .operate(Settings::list_session(session));
        self.open(session);
    }
}

fn listed(workspace: &Workspace) -> Vec<Session> {
    let editor = workspace.editor();
    let Some(held) = RootSettings::default().read(&editor) else {
        return Vec::new();
    };
    let mut sessions: Vec<Session> = held
        .settings
        .sessions()
        .into_iter()
        .filter(|session| {
            workspace
                .info(*session)
                .and_then(|info| info.metadata.get(&VIEW_EDITOR))
                == Some(WORKSPACE_EDITOR)
        })
        .map(|block| Session {
            block,
            name: workspace
                .info(block)
                .and_then(|info| info.name())
                .unwrap_or_else(|| "Session".to_owned()),
        })
        .collect();
    sessions.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.block.cmp(&right.block))
    });
    sessions
}
