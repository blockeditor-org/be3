use std::cell::RefCell;
use std::rc::Rc;

use beui::reactive::{Memo, ReadSignal, WriteSignal, create_memo, create_signal};

use crate::host::{EditorHost, FileFilter, FilePicker, PickedFile};

pub struct FileChooser<T> {
    picker: RefCell<FilePicker>,
    filter: FileFilter,
    import: Box<dyn Fn(PickedFile) -> Result<T, String>>,
    chosen: RefCell<Option<T>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    name: ReadSignal<Option<String>>,
    set_name: WriteSignal<Option<String>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

impl<T> FileChooser<T> {
    pub fn new(
        filter: FileFilter,
        import: impl Fn(PickedFile) -> Result<T, String> + 'static,
    ) -> Rc<Self> {
        let (busy, set_busy) = create_signal(false);
        let (name, set_name) = create_signal(None::<String>);
        let (error, set_error) = create_signal(None::<String>);
        Rc::new(Self {
            picker: RefCell::new(FilePicker::default()),
            filter,
            import: Box::new(import),
            chosen: RefCell::new(None),
            busy,
            set_busy,
            name,
            set_name,
            error,
            set_error,
        })
    }

    pub fn busy(&self) -> Memo<bool> {
        let busy = self.busy.clone();
        create_memo(move || busy.get())
    }

    pub fn name(&self) -> Memo<Option<String>> {
        let name = self.name.clone();
        create_memo(move || name.get())
    }

    pub fn error(&self) -> Memo<Option<String>> {
        let error = self.error.clone();
        create_memo(move || error.get())
    }

    pub fn open(&self, host: &EditorHost) {
        self.set_error.set(None);
        self.picker.borrow_mut().open(host, self.filter.clone());
        self.set_busy.set(true);
    }

    pub fn poll(&self, host: &EditorHost) {
        let result = self.picker.borrow_mut().poll(host);
        self.set_busy.set(self.picker.borrow().is_open());
        let Some(result) = result else {
            return;
        };
        let imported = result.and_then(|file| {
            let name = file.name.clone();
            (self.import)(file).map(|value| (name, value))
        });
        match imported {
            Ok((name, value)) => {
                self.set_name.set(Some(name));
                *self.chosen.borrow_mut() = Some(value);
                self.set_error.set(None);
            }
            Err(error) => {
                self.set_name.set(None);
                self.chosen.borrow_mut().take();
                self.set_error.set(Some(error));
            }
        }
    }

    pub fn take(&self) -> Option<T> {
        self.chosen.borrow_mut().take()
    }

    pub fn peek(&self) -> bool {
        self.chosen.borrow().is_some()
    }
}
