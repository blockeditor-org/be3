use beui_core::clipboard::{PasteAnswer, PasteKind};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_runner::PlatformServices;
use block_editor_plugin::accesskit::TreeUpdate;
use block_editor_plugin::driver::HostServices;

pub(crate) struct HostPlatform<'a>(pub(crate) &'a mut HostServices);

impl PlatformServices for HostPlatform<'_> {
    fn copy(&mut self, text: String) {
        self.0.copy_text(text);
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        self.0.copy_image(png);
    }

    fn paste(&mut self, _kinds: &[PasteKind], answer: PasteAnswer) {
        self.0.request_paste();
        drop(answer);
    }

    fn shows_keyboard(&self) -> bool {
        true
    }

    fn delivers_paste_shortcut(&self) -> bool {
        true
    }

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        self.0
            .pick_file(request.filter, move |pick| answer.send(pick));
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        self.0
            .save_file(request.file, move |save| answer.send(save));
    }
}
