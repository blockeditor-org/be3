use be_sdk::accesskit::TreeUpdate;
use be_sdk::driver::HostServices;
use beui_core::clipboard::{Clip, ClipData, PasteAnswer, RenderAnswer};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_runner::PlatformServices;
use beui_types::clipboard::ClipBody;

pub(crate) struct HostPlatform<'a>(pub(crate) &'a mut HostServices);

impl PlatformServices for HostPlatform<'_> {
    fn copy(&mut self, clip: Clip) {
        let items = clip
            .entries
            .iter()
            .map(|entry| be_sdk_api::OfferedClipItem {
                format: entry.format.clone(),
                body: match &entry.data {
                    ClipData::Ready(body) => Some(ClipBody::clone(body)),
                    ClipData::Lazy(_) => None,
                },
            })
            .collect();
        let render = move |format: &str, answer: RenderAnswer| {
            let entry = clip.entries.iter().find(|entry| entry.format == format);
            match entry.map(|entry| &entry.data) {
                Some(ClipData::Lazy(render)) => render(answer),
                Some(ClipData::Ready(body)) => answer(Some(ClipBody::clone(body))),
                None => answer(None),
            }
        };
        self.0.copy(items, std::rc::Rc::new(render));
    }

    fn paste(&mut self, _formats: &[String], answer: PasteAnswer) {
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
