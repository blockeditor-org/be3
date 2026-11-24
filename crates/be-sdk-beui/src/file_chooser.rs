use std::cell::{OnceCell, RefCell};

use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use crate::{Creation, Editor};
use beui::{Align, Direction, PickedFile};

const SPACING: Space = Space::Md;

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
    name: Signal<Option<String>>,
    error: Signal<Option<String>>,
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
        let name = create_signal(None::<String>);
        let error = create_signal(None::<String>);
        let chooser = Self {
            state: store(Chooser {
                filter,
                import: Box::new(import),
                picker: OnceCell::new(),
                chosen: RefCell::new(None),
                on_chosen: RefCell::new(None),
            }),
            name,
            error,
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
        self.name.read_only()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.read_only()
    }

    pub fn open(&self) {
        self.error.set(None);
        let filter = self.state.with(|state| state.filter.clone());
        self.picker().open(filter);
    }

    pub fn on_chosen(&self, chosen: impl Fn(&Self) + 'static) {
        self.state
            .with(|state| *state.on_chosen.borrow_mut() = Some(Box::new(chosen)));
    }

    fn picked(&self, picked: Result<PickedFile, String>) {
        let state = self.state.value();
        let name = picked.as_ref().ok().map(|file| file.name.clone());
        match picked.and_then(&state.import) {
            Ok(value) => {
                self.name.set(name);
                *state.chosen.borrow_mut() = Some(value);
                self.error.set(None);
            }
            Err(error) => {
                self.name.set(None);
                state.chosen.borrow_mut().take();
                self.error.set(Some(error));
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
    title: String,
    filter: FileFilter,
    import: Func<PickedFile, Result<C, String>>,
) -> NodeId
where
    C: be_block_content::BlockContent + 'static,
{
    let chooser = FileChooser::new(filter, move |file| import.call(file));
    let chosen_file = create_signal(false);
    chooser.on_chosen(move |chooser| chosen_file.set(chooser.is_chosen()));
    let creating = creation.clone();
    creation.on_create(move || {
        let chosen = chooser.take().ok_or("no file was chosen")?;
        Ok(creating.create(&chosen))
    });

    let chosen = chooser.name();
    let name = create_memo(move || chosen.get().unwrap_or_else(|| "No file chosen".to_owned()));
    let failure = chooser.error();
    let busy = chooser.busy();
    let choose = move || chooser.open();
    let choose_id = format!("{id_prefix}.choose");
    let error_id = format!("{id_prefix}.error");
    let accepted = create_signal(false);
    let accepting = creation.clone();
    let cancelling = creation;
    view! {
        <DialogBody
            title
            id="creation"
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Create", move || {
                    accepted.set(true);
                    accepting.accept();
                })
                .disabled(move || !chosen_file.get())
                .busy(accepted),
            ]}
            on_dismiss={move || cancelling.cancel()}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Button
                    label="Choose file…"
                    disabled={busy}
                    @test_id={choose_id}
                    on_click={choose}
                />
                <ListItem grow=1.0>
                    <Text style=TextStyle::CAPTION>{name}</Text>
                </ListItem>
            </List>
            <Show value={failure}>|reason: ReadSignal<String>|
                <Text style=TextStyle::CAPTION color=TextColor::Danger @test_id={error_id.clone()}>
                    {reason}
                </Text>
            </Show>
        </DialogBody>
    }
}

/// "Replace …" for a file type's editor: a button that picks a file, imports it with `import`
/// and replaces the block's content with it (`Editor::replace_content`), and the reason an
/// import failed. It is disabled while a file is read and for a reader. `on_replaced` runs
/// once the content is replaced (the PDF goes back to its first page).
#[component]
pub fn ContentFileReplace<C>(
    editor: Editor,
    label: String,
    id_prefix: String,
    filter: FileFilter,
    import: Func<PickedFile, Result<C, String>>,
    on_replaced: Option<Callback<()>>,
) -> NodeId
where
    C: be_block_content::BlockContent + 'static,
{
    let chooser = FileChooser::new(filter, move |file| import.call(file));
    chooser.on_chosen(move |chooser| {
        if let Some(replacement) = chooser.take() {
            editor.replace_content(editor.block_id(), &replacement);
            if let Some(replaced) = on_replaced {
                replaced.call(());
            }
        }
    });
    let read_only = editor.read_only();
    let busy = chooser.busy();
    let failure = chooser.error();
    let replace_id = format!("{id_prefix}.replace");
    let error_id = format!("{id_prefix}.replace-error");
    view! {
        <List spacing=SPACING>
            <Button
                label
                variant=ButtonVariant::Secondary
                disabled={move || read_only.get() || busy.get()}
                @test_id={replace_id}
                on_click={move || chooser.open()}
            />
            <Show value={failure}>|reason: ReadSignal<String>|
                <Text style=TextStyle::CAPTION color=TextColor::Danger @test_id={error_id.clone()}>
                    {reason}
                </Text>
            </Show>
        </List>
    }
}
