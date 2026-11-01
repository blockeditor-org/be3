use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use block_app_base::host::{Mailbox, Post};
use block_plugin_api::{HostImage, HostProgram, ProgramCatalog, ProgramsWatch};
use uuid::Uuid;

pub(super) const WAITING_ICON_SIZE: u32 = 64;
const ICON_BUDGET: usize = block_plugin_api::MAX_BLOB_BYTES / 2;

pub struct Opener {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
}

pub trait ProgramSource: Send + Sync + Sized + 'static {
    fn list() -> Self;

    fn programs(&self) -> Vec<HostProgram>;

    fn icon(&self, id: &str, pixels: u32) -> Option<HostImage>;

    fn run(
        &self,
        id: &str,
        launch: Option<u64>,
        file: Option<(&Path, Uuid)>,
    ) -> Option<Result<(), String>>;

    fn openers(&self) -> Vec<Opener> {
        Vec::new()
    }

    fn adopted(&self) {}
}

impl ProgramSource for () {
    fn list() -> Self {}

    fn programs(&self) -> Vec<HostProgram> {
        Vec::new()
    }

    fn icon(&self, _id: &str, _pixels: u32) -> Option<HostImage> {
        None
    }

    fn run(
        &self,
        _id: &str,
        _launch: Option<u64>,
        _file: Option<(&Path, Uuid)>,
    ) -> Option<Result<(), String>> {
        None
    }
}

enum Scanned<S> {
    Listed(Arc<S>),
    Icons(Vec<(String, HostImage)>),
    Finished,
}

struct Finished<S: ProgramSource>(Post<Scanned<S>>);

impl<S: ProgramSource> Drop for Finished<S> {
    fn drop(&mut self) {
        self.0.send(Scanned::Finished);
    }
}

struct Installed<S> {
    source: Option<Arc<S>>,
    icons: HashMap<String, HostImage>,
    post: Post<Scanned<S>>,
    _mailbox: Mailbox,
    scanning: bool,
    rescan: Option<u32>,
    waiting: Vec<(String, Option<u64>)>,
    scanned: bool,
}

pub struct Programs<S>(Rc<RefCell<Installed<S>>>);

impl<S: ProgramSource> Default for Programs<S> {
    fn default() -> Self {
        Self(Rc::new_cyclic(|installed| {
            let installed = installed.clone();
            let (mailbox, post) = block_app_base::host::mailbox(move |scanned: Scanned<S>| {
                if let Some(installed) = installed.upgrade() {
                    let installed: Rc<RefCell<Installed<S>>> = installed;
                    installed.borrow_mut().arrived(scanned);
                }
            });
            RefCell::new(Installed {
                source: None,
                icons: HashMap::new(),
                post,
                _mailbox: mailbox,
                scanning: false,
                rescan: None,
                waiting: Vec::new(),
                scanned: false,
            })
        }))
    }
}

impl<S: ProgramSource> Programs<S> {
    pub fn scan(&self, pixels: u32) {
        self.0.borrow_mut().scan(pixels);
    }

    pub fn scanned(&self) -> bool {
        self.0.borrow().scanned
    }

    pub fn announce(&self) {
        self.0.borrow().announce();
    }

    pub fn launch(&self, key: &str, launch: Option<u64>) {
        self.0.borrow_mut().launch(key, launch);
    }

    pub fn open_file(&self, key: &str, file: &Path, block: Uuid) -> Result<(), String> {
        self.0.borrow().run(key, None, Some((file, block)))
    }

    #[cfg(test)]
    fn scan_on(
        &self,
        pixels: u32,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
    ) {
        self.0.borrow_mut().scan_on(pixels, spawn);
    }

    #[cfg(test)]
    fn listed(&self) -> Vec<HostProgram> {
        self.0.borrow().listed()
    }
}

impl<S: ProgramSource> Installed<S> {
    fn scan(&mut self, pixels: u32) {
        self.scan_on(pixels, |job| {
            std::thread::Builder::new()
                .name("programs".to_owned())
                .spawn(job)
                .map(drop)
        });
    }

    fn scan_on(
        &mut self,
        pixels: u32,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
    ) {
        if self.scanning {
            self.rescan = Some(pixels);
            return;
        }
        self.scanning = true;
        let post = self.post.clone();
        let finished = Finished(post.clone());
        let spawned = spawn(Box::new(move || {
            let _finished = finished;
            let source = Arc::new(S::list());
            post.send(Scanned::Listed(Arc::clone(&source)));
            let icons = source
                .programs()
                .into_iter()
                .filter_map(|program| {
                    let icon = source.icon(&program.id, pixels)?;
                    Some((program.id, icon))
                })
                .collect();
            post.send(Scanned::Icons(icons));
        }));
        if let Err(error) = spawned {
            self.scanning = false;
            self.not_listed(error.to_string());
        }
    }

    fn arrived(&mut self, scanned: Scanned<S>) {
        match scanned {
            Scanned::Listed(source) => {
                source.adopted();
                self.source = Some(source);
                self.scanned = true;
                self.listed_now();
            }
            Scanned::Icons(icons) => {
                self.icons = icons.into_iter().collect();
                self.announce();
            }
            Scanned::Finished => {
                self.scanning = false;
                if !self.scanned {
                    self.not_listed("the scan stopped".to_owned());
                }
                if let Some(pixels) = self.rescan.take() {
                    self.scan(pixels);
                }
            }
        }
    }

    fn not_listed(&mut self, problem: String) {
        eprintln!("block-app: the programs were not listed: {problem}");
        block_app_base::notices::report(format!("Programs could not be listed: {problem}"));
        self.scanned = true;
        self.listed_now();
    }

    fn listed_now(&mut self) {
        for (key, launch) in std::mem::take(&mut self.waiting) {
            let _ = self.run(&key, launch, None);
        }
        self.announce();
        super::name_editors(&self.openers());
    }

    fn announce(&self) {
        block_app_base::watches::publish::<ProgramsWatch>(&ProgramCatalog {
            listed: self.scanned,
            programs: self.listed(),
        });
    }

    fn listed(&self) -> Vec<HostProgram> {
        let mut room = ICON_BUDGET;
        let Some(source) = &self.source else {
            return Vec::new();
        };
        source
            .programs()
            .into_iter()
            .map(|mut program| {
                program.icon = self
                    .icons
                    .get(&program.id)
                    .filter(|icon| {
                        let fits = icon.rgba.len() <= room;
                        if fits {
                            room -= icon.rgba.len();
                        }
                        fits
                    })
                    .cloned();
                program
            })
            .collect()
    }

    fn openers(&self) -> Vec<Opener> {
        self.source
            .as_ref()
            .map(|source| source.openers())
            .unwrap_or_default()
    }

    fn launch(&mut self, key: &str, launch: Option<u64>) {
        if !self.scanned {
            self.waiting.push((key.to_owned(), launch));
            self.scan(WAITING_ICON_SIZE);
            return;
        }
        let _ = self.run(key, launch, None);
    }

    fn run(
        &self,
        key: &str,
        launch: Option<u64>,
        file: Option<(&Path, Uuid)>,
    ) -> Result<(), String> {
        let ran = self
            .source
            .as_ref()
            .and_then(|source| source.run(key, launch, file));
        ran.unwrap_or_else(|| {
            eprintln!("block-app: no program {key} is installed");
            let problem = format!("{key} could not be opened: it is not installed");
            block_app_base::notices::report(problem.clone());
            Err(problem)
        })
    }
}

#[cfg(test)]
mod tests;
