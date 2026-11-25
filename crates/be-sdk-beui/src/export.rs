//! Exporting (guides/apps/editor.md, "Exporting"): an editor declares exporters, each making a
//! block of some type from its block and settings. One Export dialog runs any of them, to a
//! file, the clipboard or the workspace, where the block it makes is an artifact that the
//! exporter makes again whenever its source changes.

use std::cell::RefCell;
use std::rc::Rc;

use be_block_content::{BlockContent, LiveEdit};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{
    Artifacts, BlockParent, BlockQuery, Editor, EditorHost, ExportDialogRequest, FileSave,
    OwnedEditor, SavedFile, Task,
};
use be_block_content::BlockMetadata;

/// What an exporter made: the content of a block of its type, and what to call it. A file the
/// content becomes is named after `name` too, unless its type names it.
pub struct Made<C> {
    pub content: C,
    pub name: String,
    /// Each time the exporter makes its artifact again, the artifact is renamed `name`.
    pub renames: bool,
}

impl<C> Made<C> {
    pub fn named(content: C, name: impl Into<String>) -> Self {
        Self {
            content,
            name: name.into(),
            renames: false,
        }
    }
}

/// The settings an exporter's settings component edits: a copy, kept by whatever shows it
/// (the Export dialog, an artifact's settings dialog) until the person applies it.
pub struct ExportSettings<S: 'static> {
    value: Memo<S>,
    edit: Callback<S>,
}

impl<S: 'static> Clone for ExportSettings<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: 'static> Copy for ExportSettings<S> {}

impl<S: Clone + PartialEq + 'static> ExportSettings<S> {
    pub fn value(&self) -> Memo<S> {
        self.value
    }

    pub fn set(&self, settings: S) {
        self.edit.call(settings);
    }

    pub fn update(&self, change: impl FnOnce(&mut S)) {
        let mut settings = self.value.get_untracked();
        change(&mut settings);
        self.set(settings);
    }
}

/// What an exporter makes from its settings, once prepared on a source (`ExporterOf::new`).
pub type Maker<C, S> = Rc<dyn Fn(&S) -> Ready<Made<C>>>;

type Prepare<C, S> = Rc<dyn Fn(Editor) -> Maker<C, S>>;

/// An exporter as its editor writes it: `C` is what it makes, `S` its settings, which it keeps
/// as JSON. Hand it to `BeuiRoot::exporters` with `.into()`.
pub struct ExporterOf<C, S: 'static> {
    name: &'static str,
    prepare: Prepare<C, S>,
    #[allow(clippy::type_complexity)] // spelled out where it is used; a name would add nothing
    settings: Option<Rc<dyn Fn(ExportSettings<S>) -> Build<NodeId>>>,
    describe: Rc<dyn Fn(&S) -> String>,
    initial: Rc<dyn Fn(Editor) -> S>,
}

impl<C, S> ExporterOf<C, S>
where
    C: BlockContent + LiveEdit + Clone + Default + 'static,
    S: Serialize + DeserializeOwned + Default + Clone + PartialEq + 'static,
{
    /// The exporter its manifest declares as `name`. `prepare` runs once on each source it
    /// exports, given an editor reading it as an editor reads its own block (`block_content`,
    /// `content_of`), in a scope that lasts as long as the export: whatever state it reads the
    /// source through, it makes there. What it returns makes the content from the settings,
    /// tracked: it runs again as what it read changes, and is `Loading` until all of it arrived.
    pub fn new<M>(name: &'static str, prepare: impl Fn(Editor) -> M + 'static) -> Self
    where
        M: Fn(&S) -> Ready<Made<C>> + 'static,
    {
        Self {
            name,
            prepare: Rc::new(move |editor| Rc::new(prepare(editor)) as Maker<C, S>),
            settings: None,
            describe: Rc::new(|_| String::new()),
            initial: Rc::new(|_| S::default()),
        }
    }

    /// What edits its settings, inside whichever dialog shows them:
    /// `|settings| Build::new(move || view! { .. })`.
    pub fn settings(
        mut self,
        component: impl Fn(ExportSettings<S>) -> Build<NodeId> + 'static,
    ) -> Self {
        self.settings = Some(Rc::new(component));
        self
    }

    /// What an artifact's settings bar says it is made with.
    pub fn describe(mut self, describe: impl Fn(&S) -> String + 'static) -> Self {
        self.describe = Rc::new(describe);
        self
    }

    /// The settings an export started from the open editor (`editor.export.<name>`) starts
    /// from: what is selected, say. An export started elsewhere starts from the default.
    pub fn initial(mut self, initial: impl Fn(Editor) -> S + 'static) -> Self {
        self.initial = Rc::new(initial);
        self
    }

    /// Makes `made`, which this exporter made from `source` with `settings`, a block in the
    /// account's Unfiled: an artifact of `source` it makes again whenever `source` changes,
    /// as the Export dialog's "Add to workspace" does. Its id.
    pub fn add_to_workspace(&self, source: Editor, settings: &S, made: Made<C>) -> Uuid {
        let source_type = source
            .blocks()
            .info(source.block_id())
            .map_or(Uuid::nil(), |info| info.block_type);
        add_artifact(
            &source.host(),
            source.block_id(),
            source_type,
            self.name,
            encode(settings),
            MadeBytes {
                block_type: C::CONTENT_TYPE,
                content: made.content.encode(),
                name: made.name,
                renames: made.renames,
            },
        )
    }

    /// Makes what it makes from `source` with `settings` once, calling `done` when it is made
    /// (or could not be). The scope returned runs it: keep it until `done` is called.
    pub fn once(
        &self,
        source: Editor,
        settings: S,
        done: impl FnOnce(Result<Made<C>, String>) + 'static,
    ) -> Scope {
        let scope = Scope::root();
        let prepare = Rc::clone(&self.prepare);
        scope.run(move || {
            let make = prepare(source);
            let done = RefCell::new(Some(done));
            create_effect(move || {
                if done.borrow().is_none() {
                    return;
                }
                let result = match make(&settings) {
                    Err(Pending::Loading) => return,
                    Err(Pending::Failed(error)) => Err(error),
                    Ok(made) => Ok(made),
                };
                if let Some(done) = done.borrow_mut().take() {
                    untrack(|| done(result));
                }
            });
        });
        scope
    }
}

/// Settings as they are stored: JSON, or nothing for the default.
fn decode<S: DeserializeOwned + Default>(data: &[u8]) -> Result<S, String> {
    match data.is_empty() {
        true => Ok(S::default()),
        false => serde_json::from_slice(data)
            .map_err(|error| format!("These export settings cannot be read. {error}")),
    }
}

fn encode<S: Serialize>(settings: &S) -> Vec<u8> {
    serde_json::to_vec(settings).unwrap_or_default()
}

/// What an exporter made, encoded.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MadeBytes {
    pub(crate) block_type: Uuid,
    pub(crate) content: Vec<u8>,
    pub(crate) name: String,
    pub(crate) renames: bool,
}

type MakeBytes = Rc<dyn Fn(&[u8]) -> Ready<MadeBytes>>;

trait Export {
    fn name(&self) -> &'static str;
    fn made_type(&self) -> Uuid;
    fn initial(&self, editor: Editor) -> Vec<u8>;
    fn normalized(&self, data: &[u8]) -> Vec<u8>;
    fn describe(&self, data: &[u8]) -> Result<String, String>;
    fn settings(&self, value: Memo<Vec<u8>>, edit: Callback<Vec<u8>>) -> Option<NodeId>;
    /// Prepares it on `source`, in the current scope (`ExporterOf::new`).
    fn start(&self, source: Editor) -> MakeBytes;
}

impl<C, S> Export for ExporterOf<C, S>
where
    C: BlockContent + LiveEdit + Clone + Default + 'static,
    S: Serialize + DeserializeOwned + Default + Clone + PartialEq + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn made_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn initial(&self, editor: Editor) -> Vec<u8> {
        encode(&(self.initial)(editor))
    }

    fn normalized(&self, data: &[u8]) -> Vec<u8> {
        encode(&decode::<S>(data).unwrap_or_default())
    }

    fn describe(&self, data: &[u8]) -> Result<String, String> {
        decode::<S>(data).map(|settings| (self.describe)(&settings))
    }

    fn settings(&self, value: Memo<Vec<u8>>, edit: Callback<Vec<u8>>) -> Option<NodeId> {
        let component = self.settings.clone()?;
        let typed = create_memo(move || value.with(|data| decode::<S>(data).unwrap_or_default()));
        Some(
            component(ExportSettings {
                value: typed,
                edit: Callback::new(move |settings: S| edit.call(encode(&settings))),
            })
            .call(),
        )
    }

    fn start(&self, source: Editor) -> MakeBytes {
        let make = (self.prepare)(source);
        Rc::new(move |data: &[u8]| {
            let settings = decode::<S>(data).map_err(Pending::Failed)?;
            let made = make(&settings)?;
            Ok(MadeBytes {
                block_type: C::CONTENT_TYPE,
                content: made.content.encode(),
                name: made.name,
                renames: made.renames,
            })
        })
    }
}

/// An exporter an editor declares (`BeuiRoot::exporters`), whatever it makes.
#[derive(Clone)]
pub struct Exporter(Rc<dyn Export>);

impl<C, S> From<ExporterOf<C, S>> for Exporter
where
    C: BlockContent + LiveEdit + Clone + Default + 'static,
    S: Serialize + DeserializeOwned + Default + Clone + PartialEq + 'static,
{
    fn from(exporter: ExporterOf<C, S>) -> Self {
        Self(Rc::new(exporter))
    }
}

impl Exporter {
    pub fn name(&self) -> &'static str {
        self.0.name()
    }

    /// The block type it makes.
    pub fn made_type(&self) -> Uuid {
        self.0.made_type()
    }

    /// The settings an export from `editor`, open on its block, starts from.
    pub fn initial(&self, editor: Editor) -> Vec<u8> {
        self.0.initial(editor)
    }

    pub(crate) fn describe(&self, data: &[u8]) -> Result<String, String> {
        self.0.describe(data)
    }

    /// Prepares it on `source` in the current scope: what it returns makes what it makes from
    /// settings as stored, tracking what it reads.
    fn start(&self, source: Editor) -> MakeBytes {
        self.0.start(source)
    }
}

/// The exporter of `exporters` named `name`.
pub(crate) fn named(exporters: &[Exporter], name: &str) -> Option<Exporter> {
    exporters
        .iter()
        .find(|exporter| exporter.name() == name)
        .cloned()
}

/// The settings component of `exporter`, editing `value` through `edit`, or a line saying it
/// has none.
#[component]
pub(crate) fn ExporterSettings(
    exporter: Exporter,
    value: Memo<Vec<u8>>,
    edit: Callback<Vec<u8>>,
) -> NodeId {
    match exporter.0.settings(value, edit) {
        Some(settings) => settings,
        None => view! {
            <Text style=TextStyle::CAPTION>"This export has no settings."</Text>
        },
    }
}

/// The artifact an artifact instance keeps made (`Instance::connect_artifact`): while it runs,
/// it makes the artifact again from the settings last applied whenever its source changes,
/// and replaces the artifact's content when what it makes differs. It writes only the content,
/// the fingerprint of it (`ARTIFACT_MADE`) and the name an exporter keeps: never `ARTIFACT`,
/// which the person changes (settings applied, unlinked) while what it read of it is stale.
pub(crate) struct ArtifactRun {
    scope: Scope,
    source: OwnedEditor,
    /// Why it could not make the artifact again, as its source last changed.
    failure: Rc<RefCell<Option<String>>>,
}

impl ArtifactRun {
    pub(crate) fn start(
        artifacts: &Artifacts,
        exporter: Option<Exporter>,
        settings: &[u8],
    ) -> Self {
        let host = artifacts.host().clone();
        let source = OwnedEditor::reading(host.clone(), artifacts.source());
        let reader = source.editor();
        let target = artifacts.block_id();
        let outcome = Rc::new(RefCell::new(None::<Result<(), String>>));
        let failure = Rc::new(RefCell::new(None::<String>));
        let (told, failed) = (Rc::clone(&outcome), Rc::clone(&failure));
        let scope = Scope::root();
        let missing = format!(
            "This editor has no exporter named \"{}\".",
            artifacts.exporter()
        );
        let first = exporter
            .as_ref()
            .map_or_else(Vec::new, |exporter| exporter.0.normalized(settings));
        let (applied, asked) = scope.run(move || {
            let settings = create_signal(first);
            let again = create_trigger();
            let waiting = create_signal(false);
            let make = exporter.map(|exporter| exporter.start(reader));
            let made = create_memo(move || match &make {
                Some(make) => settings.with(|data| make(data)),
                None => Err(Pending::Failed(missing.clone())),
            });
            // Whether the block is still an artifact, and the fingerprint of what was last
            // made of it.
            let listed = reader.blocks().watch(BlockQuery::Block(target));
            let described = create_memo(move || {
                listed.read().map(|listed| {
                    listed.into_iter().next().map_or((false, None), |info| {
                        (
                            info.metadata.contains(&be_block_content::ARTIFACT),
                            info.metadata.get(&be_block_content::ARTIFACT_MADE),
                        )
                    })
                })
            });
            let writer = host.clone();
            create_effect(move || {
                again.track();
                let result = match (made.get(), described.get()) {
                    (Err(Pending::Loading), _) | (_, Err(Pending::Loading)) => return,
                    // Unlinked: it keeps what was made, and nothing is made of it again.
                    (_, Ok((false, _))) => Ok(()),
                    (Err(Pending::Failed(error)), _) | (_, Err(Pending::Failed(error))) => {
                        Err(error)
                    }
                    (Ok(made), Ok((true, last))) => {
                        untrack(|| keep(&writer, target, made, last));
                        Ok(())
                    }
                };
                *failed.borrow_mut() = result.as_ref().err().cloned();
                if waiting.get_untracked() {
                    waiting.set(false);
                    *told.borrow_mut() = Some(result);
                }
            });
            (settings, (again, waiting))
        });
        artifacts.on_regenerate(move |data| {
            let (again, waiting) = asked;
            waiting.set(true);
            applied.set(data.to_vec());
            again.notify();
        });
        artifacts.on_poll(move || outcome.borrow_mut().take());
        Self {
            scope,
            source,
            failure,
        }
    }

    /// Takes in what arrived of the source and the artifact, as a frame would: the instance is
    /// given none (`Instance::pass`).
    pub(crate) fn pass(&self) {
        let source = self.source.editor();
        // Inside an entry, so the effects woken by what arrived run as it ends.
        self.scope.run(|| source.begin_frame());
    }

    /// Why it could not make the artifact again as its source last changed, none once it did.
    pub(crate) fn failure(&self) -> Option<String> {
        self.failure.borrow().clone()
    }
}

/// A fingerprint of made content, which an artifact keeps (`ARTIFACT_MADE`) to tell whether
/// what is made again differs.
fn fingerprint(content: &[u8]) -> u64 {
    use std::hash::BuildHasher;
    be_lib_stable_hash::State::default().hash_one(content)
}

/// Writes `made` into the artifact `target`, unless it is what was made last (`last`), and
/// renames it if the exporter says to.
fn keep(host: &EditorHost, target: Uuid, made: MadeBytes, last: Option<u64>) {
    let fingerprint = fingerprint(&made.content);
    if last != Some(fingerprint) {
        host.replace_bytes(target, made.block_type, made.content);
        host.graph().define(
            target,
            be_block_content::ARTIFACT_MADE.definition(Some(&fingerprint)),
        );
    }
    if made.renames {
        rename(host, target, &made.name);
    }
}

fn rename(host: &EditorHost, block: Uuid, name: &str) {
    let name = be_block_content::metadata::clip_name(name);
    let named = host
        .graph()
        .info(block)
        .and_then(|info| info.metadata.name());
    if named.as_deref() != Some(name) {
        host.graph().set_name(block, Some(name.to_owned()));
    }
}

/// Where the Export dialog sends what it made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Destination {
    File,
    Clipboard,
    Workspace,
}

/// The Export dialog of `request.source`, in an instance opened for it
/// (`opening-kind.export`): the exporter's settings, and where to send what it makes.
#[component]
pub(crate) fn ExportDialog(
    host: EditorHost,
    source: Editor,
    exporter: Option<Exporter>,
    request: ExportDialogRequest,
) -> NodeId {
    let dismiss = host.clone();
    let dismiss_host = host.clone();
    let on_dismiss = move || dismiss.close_dialog(crate::DialogAnswer::Dismissed);
    let Some(exporter) = exporter else {
        let missing = format!(
            "This editor has no exporter named \"{}\".",
            request.exporter
        );
        return view! {
            <DialogBody
                title="Export"
                id="export"
                actions={vec![DialogAction::cancel("Close")]}
                on_dismiss={on_dismiss}
            >
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{missing}</Text>
            </DialogBody>
        };
    };
    let name = request.exporter.clone();
    let source_type = request.source_type;
    let title = create_memo(move || {
        let label = source
            .block_types()
            .exporters(source_type)
            .iter()
            .find(|listed| listed.name == name)
            .map_or_else(|| name.clone(), |listed| listed.label.clone());
        format!("Export as {label}")
    });
    let settings = create_signal(exporter.0.normalized(&request.settings));
    let value = create_memo(move || settings.get());
    let edit = Callback::new(move |data: Vec<u8>| settings.set(data));
    let wanted = create_signal(None::<Destination>);
    let busy = create_signal(false);
    let tasks = store(RefCell::new(Vec::<Task>::new()));
    // Copied from the dialog's own effect, where its document is: an answer from the host
    // arrives outside it.
    let copying = create_signal(None::<SavedFile>);
    create_effect(move || {
        if let Some(file) = copying.get() {
            untrack(|| {
                copying.set(None);
                copy(clip_of(file));
                host_done(&dismiss_host, busy);
            });
        }
    });
    let sending = Sending {
        host: host.clone(),
        source,
        source_type,
        tasks,
        busy,
        copying,
        exporter: exporter.name(),
        settings,
    };
    let make = exporter.start(source);
    create_effect(move || {
        let Some(to) = wanted.get() else {
            return;
        };
        let made = settings.with_untracked(|data| make(data));
        untrack(|| match made {
            Err(Pending::Loading) => {}
            Err(Pending::Failed(error)) => {
                wanted.set(None);
                sending.fail(&error);
            }
            Ok(made) => {
                wanted.set(None);
                sending.clone().send(to, made);
            }
        });
    });
    let send = move |to: Destination| {
        if !busy.get_untracked() {
            busy.set(true);
            wanted.set(Some(to));
        }
    };
    let working = move || busy.get();
    view! {
        <DialogBody
            title={title}
            id="export"
            actions={vec![
                DialogAction::secondary("Copy", move || send(Destination::Clipboard))
                    .named("copy")
                    .disabled(working),
                DialogAction::secondary("Add to workspace", move || send(Destination::Workspace))
                    .named("workspace")
                    .disabled(working),
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Save…", move || send(Destination::File))
                    .named("save")
                    .busy(working),
            ]}
            on_dismiss={on_dismiss}
        >
            <ExporterSettings exporter value edit />
        </DialogBody>
    }
}

/// Where the Export dialog sends what it made, and how it says it is done.
#[derive(Clone)]
struct Sending {
    host: EditorHost,
    source: Editor,
    source_type: Uuid,
    tasks: Stored<RefCell<Vec<Task>>>,
    busy: Signal<bool>,
    copying: Signal<Option<SavedFile>>,
    exporter: &'static str,
    settings: Signal<Vec<u8>>,
}

impl Sending {
    fn send(self, to: Destination, made: MadeBytes) {
        match to {
            Destination::Workspace => self.make_artifact(made),
            Destination::File | Destination::Clipboard => {
                let asked = self
                    .host
                    .file_of_content(made.block_type, made.content, made.name);
                let filed = self.clone();
                // The host's own `then`: the dialog keeps the task, and an effect's run that
                // asked is gone by the time the answer comes.
                let task = self.host.then(asked, move |file| match file {
                    Ok(file) if to == Destination::File => filed.save(file),
                    Ok(file) => filed.copying.set(Some(file)),
                    Err(failure) => filed.fail(&failure.to_string()),
                });
                self.keep(task);
            }
        }
    }

    fn save(self, file: SavedFile) {
        let asked = self.host.save_file(file);
        let saved = self.clone();
        let task = self.host.then(asked, move |save| match save {
            FileSave::Saved => saved.done(),
            FileSave::Cancelled => saved.busy.set(false),
            FileSave::Failed(failure) => saved.fail(&failure),
        });
        self.keep(task);
    }

    /// Makes the block in the account's Unfiled (`add_artifact`), and opens it.
    fn make_artifact(self, made: MadeBytes) {
        let block_type = made.block_type;
        let block = add_artifact(
            &self.host,
            self.source.block_id(),
            self.source_type,
            self.exporter,
            self.settings.get_untracked(),
            made,
        );
        self.host.open_block(block, block_type);
        self.done();
    }

    fn keep(&self, task: Task) {
        self.tasks.with(|tasks| tasks.borrow_mut().push(task));
    }

    fn done(&self) {
        host_done(&self.host, self.busy);
    }

    fn fail(&self, error: &str) {
        self.busy.set(false);
        self.host
            .report_failure(format!("The block could not be exported. {error}"));
    }
}

/// Makes `made` a block in the account's Unfiled, as an artifact of `source` that `exporter`
/// makes again from `settings` whenever `source` changes. Its id.
fn add_artifact(
    host: &EditorHost,
    source: Uuid,
    source_type: Uuid,
    exporter: &str,
    settings: Vec<u8>,
    made: MadeBytes,
) -> Uuid {
    let mut metadata = BlockMetadata::named(be_block_content::metadata::clip_name(&made.name));
    metadata.insert(
        &be_block_content::ARTIFACT,
        &be_block_content::Artifact {
            source: source.into(),
            source_type,
            exporter: exporter.to_owned(),
            settings,
        },
    );
    metadata.insert(
        &be_block_content::ARTIFACT_MADE,
        &fingerprint(&made.content),
    );
    host.graph().create_with(
        made.block_type,
        Some(made.content),
        BlockParent::Unfiled,
        metadata,
    )
}

/// Closes the Export dialog: what it made has gone where it was sent.
fn host_done(host: &EditorHost, busy: Signal<bool>) {
    busy.set(false);
    host.close_dialog(crate::DialogAnswer::Accepted);
}

/// `file` as the clipboard holds it: its own media type, and as text too when it is text.
fn clip_of(file: SavedFile) -> Clip {
    let textual = file.mime_type.starts_with("text/")
        || file.mime_type.ends_with("+xml")
        || file.mime_type.ends_with("+json")
        || file.mime_type == "application/json";
    let text = textual
        .then(|| String::from_utf8(file.data.clone()).ok())
        .flatten();
    let mut clip = Clip::new().with(ClipItem::new(file.mime_type, file.data));
    if let Some(text) = text {
        clip = clip.with(ClipItem::text(text));
    }
    clip
}
