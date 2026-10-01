use std::cell::RefCell;
use std::rc::{Rc, Weak};

use beui::NodeId;
use beui::reactive::{
    Align, Direction, FileFilter, FilePicker, Frame, Func, ItemSize, List, PickedFile, ReadSignal,
    Show, Spacer, WriteSignal, clone, component, create_file_picker, create_memo, create_signal,
    view,
};
use beui::styled::{Button, ButtonVariant, Caption, use_theme};

use crate::Creation;

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;

type Import<T> = Box<dyn Fn(PickedFile) -> Result<T, String>>;

type Chosen<T> = Box<dyn Fn(&FileChooser<T>)>;

pub struct FileChooser<T> {
    filter: FileFilter,
    import: Import<T>,
    picker: FilePicker,
    chosen: RefCell<Option<T>>,
    on_chosen: RefCell<Option<Chosen<T>>>,
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
        let (name, set_name) = create_signal(None::<String>);
        let (error, set_error) = create_signal(None::<String>);
        Rc::new_cyclic(|chooser: &Weak<Self>| {
            let chooser = chooser.clone();
            Self {
                filter,
                import: Box::new(import),
                picker: create_file_picker(move |picked| {
                    if let Some(chooser) = chooser.upgrade() {
                        chooser.picked(picked);
                    }
                }),
                chosen: RefCell::new(None),
                on_chosen: RefCell::new(None),
                name,
                set_name,
                error,
                set_error,
            }
        })
    }

    pub fn busy(&self) -> ReadSignal<bool> {
        self.picker.picking()
    }

    pub fn name(&self) -> ReadSignal<Option<String>> {
        self.name.clone()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.clone()
    }

    pub fn open(&self) {
        self.set_error.set(None);
        self.picker.open(self.filter.clone());
    }

    pub fn on_chosen(&self, chosen: impl Fn(&Self) + 'static) {
        *self.on_chosen.borrow_mut() = Some(Box::new(chosen));
    }

    fn picked(&self, picked: Result<PickedFile, String>) {
        let name = picked.as_ref().ok().map(|file| file.name.clone());
        match picked.and_then(&self.import) {
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
        if let Some(chosen) = self.on_chosen.borrow().as_ref() {
            chosen(self);
        }
    }

    pub fn take(&self) -> Option<T> {
        self.chosen.borrow_mut().take()
    }

    pub fn is_chosen(&self) -> bool {
        self.chosen.borrow().is_some()
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
    let made = Rc::clone(&chooser);
    let creating = creation.clone();
    creation.on_create(move || {
        let chosen = made.take().ok_or("no file was chosen")?;
        Ok(creating.create(&chosen))
    });

    let chosen = chooser.name();
    let name = create_memo(clone!(chosen -> move || {
        chosen.get().unwrap_or_else(|| "No file chosen".to_owned())
    }));
    let failure = chooser.error();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let busy = chooser.busy();
    let choose = move || chooser.open();
    let choose_id = format!("{id_prefix}.choose");
    let error_id = format!("{id_prefix}.error");
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
                    <Caption
                        content={reason}
                        color={theme.danger.clone()}
                        wrap=true
                        @test_id={error_id}
                    />
                </Show>
            </List>
        </Frame>
    }
}
