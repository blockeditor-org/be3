use beui_core::clipboard::{PasteAnswer, PasteKind};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_core::runner::Platform;
use block_editor_plugin::EditorHost;
use block_editor_plugin::accesskit::TreeUpdate;

pub(crate) struct HostPlatform {
    host: EditorHost,
    tree: Option<TreeUpdate>,
}

impl HostPlatform {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self { host, tree: None }
    }

    pub(crate) fn take_tree(&mut self) -> Option<TreeUpdate> {
        self.tree.take()
    }
}

impl Platform for HostPlatform {
    fn copy(&mut self, text: String) {
        self.host.copy_text(text);
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        self.host.copy_image(png);
    }

    fn paste(&mut self, _kinds: &[PasteKind], answer: PasteAnswer) {
        self.host.request_paste();
        drop(answer);
    }

    fn shows_keyboard(&self) -> bool {
        true
    }

    fn delivers_paste_shortcut(&self) -> bool {
        true
    }

    fn publish_accessibility(&mut self, tree: &mut dyn FnMut() -> TreeUpdate) {
        self.tree = Some(tree());
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        let asked = self.host.pick_file(request.filter);
        self.host.then(asked, move |pick| answer.send(pick));
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        let asked = self.host.save_file(request.file);
        self.host.then(asked, move |save| answer.send(save));
    }
}
