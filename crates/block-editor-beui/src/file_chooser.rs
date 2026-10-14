use std::cell::{OnceCell, RefCell};

use beui::NodeId;
use beui::reactive::{
    Align, Direction, FileFilter, FilePicker, Frame, Func, ItemSize, List, PickedFile, ReadSignal,
    Show, Spacer, Stored, WriteSignal, component, create_file_picker, create_memo, create_signal,
    store, view,
};
use beui::styled::{Button, ButtonVariant};

use crate::Creation;
use beui::styled::{Text, TextColor, TextStyle};

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;

type Import<T> = Box<dyn Fn(PickedFile) -> Result<T, String>>;

type Chosen<T> = Box<dyn Fn(&FileChooser<T>)>;

struct Chooser<T: 'static> {
    filter: FileFilter,
    import: Import<T>,
    picker: OnceCell<FilePicker>,
    chosen: RefCell<Option<T>>,
    on_chosen: RefCell<Option<Chosen<T>>>,
}

pub struct FileChooser<T: 'static> {
    state: Stored<Chooser<T>>,
    name: ReadSignal<Option<String>>,
    set_name: WriteSignal<Option<String>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

impl<T> Clone for FileChooser<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for FileChooser<T> {}

impl<T: 'static> FileChooser<T> {
    pub fn new(
        filter: FileFilter,
        import: impl Fn(PickedFile) -> Result<T, String> + 'static,
    ) -> Self {
        let (name, set_name) = create_signal(None::<String>);
        let (error, set_error) = create_signal(None::<String>);
        let chooser = Self {
            state: store(Chooser {
                filter,
                import: Box::new(import),
                picker: OnceCell::new(),
                chosen: RefCell::new(None),
                on_chosen: RefCell::new(None),
            }),
            name,
            set_name,
            error,
            set_error,
        };
        let picker = create_file_picker(move |picked| {
            if chooser.state.is_alive() {
                chooser.picked(picked);
            }
        });
        chooser.state.with(|state| state.picker.set(picker).ok());
        chooser
    }

    fn picker(&self) -> FilePicker {
        self.state
            .with(|state| state.picker.get().cloned())
            .expect("a file chooser has its picker from the start")
    }

    pub fn busy(&self) -> ReadSignal<bool> {
        self.picker().picking()
    }

    pub fn name(&self) -> ReadSignal<Option<String>> {
        self.name
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub fn open(&self) {
        self.set_error.set(None);
        let filter = self.state.with(|state| state.filter.clone());
        self.picker().open(filter);
    }

    pub fn on_chosen(&self, chosen: impl Fn(&Self) + 'static) {
        self.state
            .with(|state| *state.on_chosen.borrow_mut() = Some(Box::new(chosen)));
    }

    fn picked(&self, picked: Result<PickedFile, String>) {
        let state = self.state.get();
        let name = picked.as_ref().ok().map(|file| file.name.clone());
        match picked.and_then(&state.import) {
            Ok(value) => {
                self.set_name.set(name);
                *state.chosen.borrow_mut() = Some(value);
                self.set_error.set(None);
            }
            Err(error) => {
                self.set_name.set(None);
                state.chosen.borrow_mut().take();
                self.set_error.set(Some(error));
            }
        }
        if let Some(chosen) = state.on_chosen.borrow().as_ref() {
            chosen(self);
        }
    }

    pub fn take(&self) -> Option<T> {
        self.state
            .try_with(|state| state.chosen.borrow_mut().take())
            .flatten()
    }

    pub fn is_chosen(&self) -> bool {
        self.state.with(|state| state.chosen.borrow().is_some())
    }
}

#[component]
pub fn ContentFileCreation<C>(
    creation: Creation,
    id_prefix: String,
    filter: FileFilter,
    import: Func<PickedFile, Result<C, String>>,
) -> NodeId
where
    C: be_block::BlockContent + 'static,
{
    let chooser = FileChooser::new(filter, move |file| import.call(file));
    let host = creation.host().clone();
    chooser.on_chosen(move |chooser| host.set_creation_ready(chooser.is_chosen()));
    let creating = creation.clone();
    creation.on_create(move || {
        let chosen = chooser.take().ok_or("no file was chosen")?;
        Ok(creating.create(&chosen))
    });

    let chosen = chooser.name();
    let name = create_memo(move || chosen.get().unwrap_or_else(|| "No file chosen".to_owned()));
    let failure = chooser.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let busy = chooser.busy();
    let choose = move || chooser.open();
    let choose_id = format!("{id_prefix}.choose");
    let error_id = format!("{id_prefix}.error");
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
                    <Text style=TextStyle::CAPTION string={name} />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <Show condition={failed}>
                    <Text
                        style=TextStyle::CAPTION
                        string={reason}
                        color=TextColor::Danger
                        wrap=true
                        @test_id={error_id.clone()}
                    />
                </Show>
            </List>
        </Frame>
    }
}
