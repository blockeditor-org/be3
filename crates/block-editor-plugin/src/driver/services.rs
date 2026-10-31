use block_plugin_api::{EditorInstanceId, FilePick, FileSave};

use crate::{EditorHost, FileFilter, SavedFile};

pub struct HostServices {
    launched: EditorHost,
    host: EditorHost,
}

impl HostServices {
    pub fn new(host: EditorHost) -> Self {
        Self {
            launched: host.clone(),
            host,
        }
    }

    pub fn host(&self) -> &EditorHost {
        &self.host
    }

    pub fn serve(&mut self, instance: EditorInstanceId) {
        if let Some(host) = self
            .launched
            .module()
            .and_then(|module| module.host(instance))
        {
            self.host = host;
        }
    }

    pub fn copy_text(&mut self, text: String) {
        self.host.copy_text(text);
    }

    pub fn copy_image(&mut self, png: Vec<u8>) {
        self.host.copy_image(png);
    }

    pub fn request_paste(&mut self) {
        self.host.request_paste();
    }

    pub fn pick_file(&mut self, filter: FileFilter, answer: impl FnOnce(FilePick) + 'static) {
        let asked = self.host.pick_file(filter);
        self.host.then(asked, answer);
    }

    pub fn save_file(&mut self, file: SavedFile, answer: impl FnOnce(FileSave) + 'static) {
        let asked = self.host.save_file(file);
        self.host.then(asked, answer);
    }
}
