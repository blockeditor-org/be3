use std::cell::RefCell;
use std::rc::Rc;

use beui::NodeId;
use beui::reactive::{
    Align, Direction, Frame, ItemSize, List, ReadSignal, Show, Spacer, WriteSignal, clone,
    create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Caption, use_theme};

use crate::{Creation, EditorHost, FileFilter, FilePicker, PickedFile};

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;

type Import<T> = Box<dyn Fn(PickedFile) -> Result<T, String>>;

pub struct FileChooser<T> {
    filter: FileFilter,
    import: Import<T>,
    picker: RefCell<FilePicker>,
    chosen: RefCell<Option<T>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    name: ReadSignal<Option<String>>,
    set_name: WriteSignal<Option<String>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

impl<T: 'static> FileChooser<T> {
    pub fn new(
        filter: FileFilter,
        import: impl Fn(PickedFile) -> Result<T, String> + 'static,
    ) -> Rc<Self> {
        let (busy, set_busy) = create_signal(false);
        let (name, set_name) = create_signal(None::<String>);
        let (error, set_error) = create_signal(None::<String>);
        Rc::new(Self {
            filter,
            import: Box::new(import),
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

    pub fn busy(&self) -> ReadSignal<bool> {
        self.busy.clone()
    }

    pub fn name(&self) -> ReadSignal<Option<String>> {
        self.name.clone()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.clone()
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
        let name = result.as_ref().ok().map(|file| file.name.clone());
        match result.and_then(&self.import) {
            Ok(value) => {
                self.set_name.set(name);
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

    pub fn is_chosen(&self) -> bool {
        self.chosen.borrow().is_some()
    }
}

fn file_creation_with<T: 'static>(
    creation: &Creation,
    test_id: &str,
    filter: FileFilter,
    import: impl Fn(PickedFile) -> Result<T, String> + 'static,
    make: impl Fn(T) -> uuid::Uuid + 'static,
) -> NodeId {
    let chooser = FileChooser::new(filter, import);
    let polled = Rc::clone(&chooser);
    let host = creation.host().clone();
    creation.each_frame(move || {
        polled.poll(&host);
        host.set_creation_ready(polled.is_chosen());
    });
    let made = Rc::clone(&chooser);
    creation.on_create(move || {
        let chosen = made.take().ok_or("no file was chosen")?;
        Ok(make(chosen))
    });

    let chosen = chooser.name();
    let name = create_memo(clone!(chosen -> move || {
        chosen.get().unwrap_or_else(|| "No file chosen".to_owned())
    }));
    let failure = chooser.error();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let busy = chooser.busy();
    let opening = creation.host().clone();
    let choose = move || chooser.open(&opening);
    let choose_id = format!("{test_id}.choose");
    let error_id = format!("{test_id}.error");
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        label="Choose file…"
                        variant=ButtonVariant::Primary
                        disabled={busy}
                        @test_id={choose_id}
                        on_click={choose}
                    />
                    <Caption content={name} />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <Show condition={failed}>
                    <Caption content={reason} color={theme.danger.clone()} @test_id={error_id} />
                </Show>
            </List>
        </Frame>
    }
}

pub fn content_file_creation<C: be_block::BlockContent>(
    creation: &Creation,
    test_id: &str,
    filter: FileFilter,
    import: impl Fn(PickedFile) -> Result<C, String> + 'static,
) -> NodeId {
    let creating = creation.clone();
    file_creation_with(creation, test_id, filter, import, move |content: C| {
        creating.create(&content)
    })
}
