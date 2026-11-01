mod files;
mod installed;
#[cfg(target_os = "linux")]
mod watch;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use block_plugin_api::{EditorDescriptor, Edits, HostWindowId, ProgramAction};
use uuid::Uuid;

use crate::desktop_host::{DesktopHost, Host};

use files::OnDisk;
use installed::WAITING_ICON_SIZE;
pub use installed::{Opener, ProgramSource};

type Installed = installed::Programs<<Host as DesktopHost>::Source>;

const PROGRAM_EDITORS: u64 = u64::from_be_bytes(*b"program\0");

pub fn editor_id(program: &str) -> Uuid {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in program.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Uuid::from_u64_pair(PROGRAM_EDITORS, hash)
}

pub fn is_program(editor: Uuid) -> bool {
    editor.as_u64_pair().0 == PROGRAM_EDITORS
}

fn descriptors(openers: &[Opener]) -> Vec<EditorDescriptor> {
    openers
        .iter()
        .map(|opener| EditorDescriptor {
            editor: editor_id(&opener.id).into_bytes(),
            edits: Edits::Any,
            name: opener.name.clone(),
            icon_codepoint: beui::icons::ICON_APP_WINDOW.as_str().to_owned(),
            extensions: opener.extensions.clone(),
        })
        .collect()
}

fn name_editors(openers: &[Opener]) {
    with(|sessions| {
        sessions.names = openers
            .iter()
            .map(|opener| {
                (
                    editor_id(&opener.id),
                    (opener.id.clone(), opener.name.clone()),
                )
            })
            .collect();
    });
    block_app_base::program_editors::set(descriptors(openers));
    block_app_base::host::changed();
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    Opening(String),
    Window(HostWindowId),
    Closed(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Wanted,
    Reading,
    Placed,
    Launching,
    Running(HostWindowId),
    Closed,
    Failed(String),
}

struct Session {
    parent: Uuid,
    block_type: Uuid,
    editor: Uuid,
    stage: Stage,
    disk: Option<OnDisk>,
    closing: bool,
}

#[derive(Default)]
struct Sessions {
    open: HashMap<Uuid, Session>,
    names: HashMap<Uuid, (String, String)>,
}

thread_local! {
    static SESSIONS: RefCell<Sessions> = RefCell::new(Sessions::default());
}

fn with<R>(act: impl FnOnce(&mut Sessions) -> R) -> R {
    SESSIONS.with(|sessions| act(&mut sessions.borrow_mut()))
}

pub fn opens(editor: Uuid) -> bool {
    is_program(editor) && with(|sessions| sessions.names.contains_key(&editor))
}

pub fn show(block: Uuid, block_type: Uuid, editor: Uuid, parent: Uuid) {
    let replaced = with(|sessions| {
        if let Some(session) = sessions.open.get_mut(&block)
            && session.editor == editor
        {
            session.closing = false;
            return None;
        }
        let old = sessions.open.insert(
            block,
            Session {
                parent,
                block_type,
                editor,
                stage: Stage::Wanted,
                disk: None,
                closing: false,
            },
        );
        Some(old.and_then(|old| window_of(&old.stage)))
    });
    if let Some(window) = replaced.flatten() {
        Host::close(window);
    }
    if replaced.is_some() {
        block_app_base::host::changed();
    }
}

pub fn shown(block: Uuid) -> Option<Shown> {
    with(|sessions| {
        let session = sessions.open.get(&block)?;
        let name = sessions
            .names
            .get(&session.editor)
            .map_or_else(|| "its program".to_owned(), |(_, name)| name.clone());
        Some(match &session.stage {
            Stage::Running(window) => Shown::Window(*window),
            Stage::Closed => Shown::Closed(name),
            Stage::Failed(problem) => Shown::Failed(problem.clone()),
            Stage::Wanted | Stage::Reading | Stage::Placed | Stage::Launching => {
                Shown::Opening(name)
            }
        })
    })
}

pub fn reopen(block: Uuid) {
    with(|sessions| {
        if let Some(session) = sessions.open.get_mut(&block)
            && matches!(session.stage, Stage::Closed | Stage::Failed(_))
        {
            session.stage = Stage::Wanted;
        }
    });
    block_app_base::host::changed();
}

pub fn close(closed: Uuid) {
    let windows = with(|sessions| {
        let inside: Vec<Uuid> = sessions
            .open
            .iter()
            .filter(|(block, session)| **block == closed || session.parent == closed)
            .map(|(block, _)| *block)
            .collect();
        let mut windows = Vec::new();
        for block in inside {
            let Some(session) = sessions.open.get_mut(&block) else {
                continue;
            };
            match session.stage {
                Stage::Running(window) => {
                    session.closing = true;
                    windows.push(window);
                }
                Stage::Launching => session.closing = true,
                _ => {
                    sessions.open.remove(&block);
                }
            }
        }
        windows
    });
    for window in windows {
        Host::close(window);
    }
}

pub fn bound_windows() -> Vec<HostWindowId> {
    with(|sessions| {
        let mut bound: Vec<HostWindowId> = sessions
            .open
            .values()
            .filter_map(|session| window_of(&session.stage))
            .collect();
        bound.sort();
        bound
    })
}

fn window_of(stage: &Stage) -> Option<HostWindowId> {
    match stage {
        Stage::Running(window) => Some(*window),
        _ => None,
    }
}

type Read = (Uuid, Result<be_types_host::PlainFile, String>);

pub struct Programs {
    programs: Installed,
    offered: bool,
    files: Option<PathBuf>,
    read: (block_app_base::host::WakingSender<Read>, Receiver<Read>),
    #[cfg(target_os = "linux")]
    watch: Option<watch::Watch>,
}

impl Default for Programs {
    fn default() -> Self {
        Self {
            programs: Installed::default(),
            offered: false,
            files: None,
            read: block_app_base::host::waking_channel(),
            #[cfg(target_os = "linux")]
            watch: None,
        }
    }
}

impl Programs {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn keep_files_in(&mut self, files: PathBuf) {
        self.files = Some(files);
    }

    pub fn frame(&mut self) {
        let running = Host::running();
        for action in block_app_base::watches::take_calls(block_plugin_api::requested::program) {
            match action {
                ProgramAction::List { icon_size } if running => {
                    self.programs
                        .scan(icon_size.clamp(1, ProgramAction::MAX_ICON_SIZE));
                }
                ProgramAction::Launch { program, launch } if running => {
                    self.programs.launch(&program, launch);
                }
                ProgramAction::Run(line) => {
                    Host::launch(line);
                }
                ProgramAction::Exec { command, launch } => {
                    Host::exec(command, launch);
                }
                ProgramAction::List { .. } | ProgramAction::Launch { .. } => {}
            }
        }
        if running && !self.offered {
            self.offered = true;
            self.programs.announce();
            if self.files.is_some() {
                self.programs.scan(WAITING_ICON_SIZE);
            }
        }
        self.step_sessions();
    }

    fn step_sessions(&mut self) {
        let Some(files) = self.files.clone() else {
            return;
        };
        let mut changed = false;
        let wanted: Vec<Uuid> = with(|sessions| {
            sessions
                .open
                .iter_mut()
                .filter(|(_, session)| session.stage == Stage::Wanted)
                .map(|(block, session)| {
                    session.stage = Stage::Reading;
                    *block
                })
                .collect()
        });
        for block in wanted {
            let sender = self.read.0.clone();
            block_app_stack::plain_file(block, move |read| {
                let _ = sender.send((block, read));
            });
            changed = true;
        }
        while let Ok((block, read)) = self.read.1.try_recv() {
            changed = true;
            self.placed(&files, block, read);
        }
        if self.programs.scanned() {
            changed |= self.launch_placed();
        }
        changed |= self.bind_windows();
        self.write_back_touched();
        if changed {
            block_app_base::host::changed();
        }
    }

    fn placed(
        &mut self,
        files: &Path,
        block: Uuid,
        read: Result<be_types_host::PlainFile, String>,
    ) {
        let block_type = with(|sessions| {
            sessions
                .open
                .get(&block)
                .filter(|session| session.stage == Stage::Reading)
                .map(|session| session.block_type)
        });
        let Some(block_type) = block_type else {
            return;
        };
        let stage = match read.and_then(|file| files::place(files, block, &file)) {
            Ok(placed) => {
                if let Some((name, bytes)) = placed.taken_back {
                    block_app_stack::from_file(block, block_type, name, bytes);
                }
                #[cfg(target_os = "linux")]
                self.watch
                    .get_or_insert_with(watch::Watch::new)
                    .add(&placed.disk.path);
                with(|sessions| {
                    if let Some(session) = sessions.open.get_mut(&block) {
                        session.disk = Some(placed.disk);
                    }
                });
                Stage::Placed
            }
            Err(problem) => {
                eprintln!(
                    "block-app: block {block} was not written out for its program: {problem}"
                );
                Stage::Failed(problem)
            }
        };
        with(|sessions| {
            if let Some(session) = sessions.open.get_mut(&block) {
                session.stage = stage;
            }
        });
    }

    fn launch_placed(&mut self) -> bool {
        let ready: Vec<(Uuid, Option<String>, PathBuf)> = with(|sessions| {
            sessions
                .open
                .iter()
                .filter(|(_, session)| session.stage == Stage::Placed)
                .filter_map(|(block, session)| {
                    let key = sessions
                        .names
                        .get(&session.editor)
                        .map(|(id, _)| id.clone());
                    Some((*block, key, session.disk.as_ref()?.path.clone()))
                })
                .collect()
        });
        let launched = !ready.is_empty();
        for (block, key, path) in ready {
            let stage = match key {
                Some(key) => match self.programs.open_file(&key, &path, block) {
                    Ok(()) => Stage::Launching,
                    Err(problem) => Stage::Failed(problem),
                },
                None => Stage::Failed("The program this opens in is not installed.".to_owned()),
            };
            with(|sessions| {
                if let Some(session) = sessions.open.get_mut(&block) {
                    session.stage = stage;
                }
            });
        }
        launched
    }

    fn bind_windows(&mut self) -> bool {
        let opening = Host::opening_windows();
        let mut gone = Vec::new();
        let mut unwanted = Vec::new();
        let changed = with(|sessions| {
            let mut changed = false;
            for (block, session) in &mut sessions.open {
                let window = opening
                    .iter()
                    .find(|(opened, _)| opened == block)
                    .map(|(_, window)| *window);
                match (&session.stage, window) {
                    (Stage::Launching, Some(window)) => {
                        session.stage = Stage::Running(window);
                        if session.closing {
                            unwanted.push(window);
                        }
                        changed = true;
                    }
                    (Stage::Running(held), Some(window)) if *held != window => {
                        session.stage = Stage::Running(window);
                        changed = true;
                    }
                    (Stage::Running(_), None) => {
                        session.stage = Stage::Closed;
                        gone.push(*block);
                        changed = true;
                    }
                    _ => {}
                }
            }
            changed
        });
        for window in unwanted {
            Host::close(window);
        }
        for block in gone {
            self.write_back(block);
            let closing = with(|sessions| {
                let closing = sessions
                    .open
                    .get(&block)
                    .is_some_and(|session| session.closing);
                if closing {
                    sessions.open.remove(&block);
                }
                closing
            });
            if closing {
                #[cfg(target_os = "linux")]
                if let Some(watch) = &mut self.watch {
                    watch.forget(block);
                }
            }
        }
        changed
    }

    fn write_back_touched(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let touched = self
                .watch
                .as_mut()
                .map(watch::Watch::take)
                .unwrap_or_default();
            if touched.is_empty() {
                return;
            }
            let blocks: Vec<Uuid> = with(|sessions| {
                sessions
                    .open
                    .iter()
                    .filter(|(_, session)| {
                        session
                            .disk
                            .as_ref()
                            .is_some_and(|disk| touched.contains(&disk.path))
                    })
                    .map(|(block, _)| *block)
                    .collect()
            });
            for block in blocks {
                self.write_back(block);
            }
        }
    }

    fn write_back(&mut self, block: Uuid) {
        let held = with(|sessions| {
            let session = sessions.open.get(&block)?;
            Some((session.block_type, session.disk.clone()?))
        });
        let Some((block_type, disk)) = held else {
            return;
        };
        let Some((name, bytes, stamp)) = files::changed(&disk) else {
            return;
        };
        block_app_stack::from_file(block, block_type, name, bytes);
        if let Err(error) = files::record(&disk.path, stamp) {
            eprintln!(
                "block-app: what was taken back from {} was not recorded: {error}",
                disk.path.display()
            );
        }
        with(|sessions| {
            if let Some(disk) = sessions
                .open
                .get_mut(&block)
                .and_then(|session| session.disk.as_mut())
            {
                disk.stamp = stamp;
            }
        });
    }
}
