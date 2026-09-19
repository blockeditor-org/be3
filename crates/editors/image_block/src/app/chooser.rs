use std::cell::RefCell;
use std::rc::Rc;

use block_client::blocks::image::Image as ImageBlock;
use block_editor_plugin::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_plugin::{EditorHost, FileFilter, FilePicker, PickedFile};

pub(crate) struct Chooser {
    picker: RefCell<FilePicker>,
    chosen: RefCell<Option<ImageBlock>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    name: ReadSignal<Option<String>>,
    set_name: WriteSignal<Option<String>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

impl Chooser {
    pub(crate) fn new() -> Rc<Self> {
        let (busy, set_busy) = create_signal(false);
        let (name, set_name) = create_signal(None::<String>);
        let (error, set_error) = create_signal(None::<String>);
        Rc::new(Self {
            picker: RefCell::new(FilePicker::default()),
            chosen: RefCell::new(None),
            busy,
            set_busy,
            name,
            set_name,
            error,
            set_error,
        })
    }

    pub(crate) fn busy(&self) -> Memo<bool> {
        let busy = self.busy.clone();
        create_memo(move || busy.get())
    }

    pub(crate) fn name(&self) -> Memo<Option<String>> {
        let name = self.name.clone();
        create_memo(move || name.get())
    }

    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error.clone();
        create_memo(move || error.get())
    }

    pub(crate) fn open(&self, host: &EditorHost) {
        self.set_error.set(None);
        self.picker.borrow_mut().open(host, filter());
        self.set_busy.set(true);
    }

    pub(crate) fn poll(&self, host: &EditorHost) {
        let result = self.picker.borrow_mut().poll(host);
        self.set_busy.set(self.picker.borrow().is_open());
        let Some(result) = result.map(|file| file.and_then(imported)) else {
            return;
        };
        match result {
            Ok(image) => {
                self.set_name.set(Some(image.source_name().to_owned()));
                *self.chosen.borrow_mut() = Some(image);
                self.set_error.set(None);
            }
            Err(error) => {
                self.set_name.set(None);
                self.chosen.borrow_mut().take();
                self.set_error.set(Some(error));
            }
        }
    }

    pub(crate) fn take(&self) -> Option<ImageBlock> {
        self.chosen.borrow_mut().take()
    }

    pub(crate) fn peek(&self) -> bool {
        self.chosen.borrow().is_some()
    }
}

fn filter() -> FileFilter {
    FileFilter {
        name: "Images".to_owned(),
        default_file_name: "Image".to_owned(),
        extensions: ImageBlock::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: ImageBlock::MIME_TYPES
            .iter()
            .map(|mime_type| (*mime_type).to_owned())
            .collect(),
    }
}

fn imported(file: PickedFile) -> Result<ImageBlock, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageBlock::new(name, data))
}
