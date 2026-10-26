use std::collections::HashSet;
use std::path::{Path, PathBuf};

const UNCLAIMED_KEPT: usize = 32;

/// A program block-app started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    /// The process `ShellExecuteExW` started, when it says; a shortcut that
    /// hands over to a running process (a browser's second window) gives one
    /// that exits, or none.
    pub process_id: Option<u32>,
    /// The program's id in the program list.
    pub program: Option<String>,
    pub command: Vec<String>,
    /// The caller's number for this launch, echoed in the windows it opens.
    pub launch: Option<u64>,
    /// The executable the shortcut points at.
    pub target: Option<PathBuf>,
}

/// Where a window came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Origin {
    pub program: Option<String>,
    pub command: Vec<String>,
    pub launch: Option<u64>,
}

#[derive(Clone, Debug)]
struct Started {
    launch: Launch,
    claimed: bool,
}

/// The launches whose windows have not all closed, matched to windows by
/// process id, else by executable.
#[derive(Clone, Debug, Default)]
pub struct Launches {
    started: Vec<Started>,
}

impl Launches {
    pub fn add(&mut self, launch: Launch) {
        self.started.push(Started {
            launch,
            claimed: false,
        });
        let unclaimed = self.started.iter().filter(|started| !started.claimed).count();
        if unclaimed > UNCLAIMED_KEPT
            && let Some(oldest) = self.started.iter().position(|started| !started.claimed)
        {
            self.started.remove(oldest);
        }
    }

    /// The origin of a window that just appeared: the latest launch of its
    /// process, else the oldest unclaimed launch of its executable, which
    /// then belongs to its process. A window no launch started comes from
    /// its executable alone.
    pub fn origin(&mut self, process_id: u32, exe: Option<&Path>) -> Origin {
        let by_process = self
            .started
            .iter()
            .rposition(|started| started.launch.process_id == Some(process_id));
        let by_target = || {
            let exe = exe?;
            self.started.iter().position(|started| {
                !started.claimed
                    && started
                        .launch
                        .target
                        .as_deref()
                        .is_some_and(|target| same_path(target, exe))
            })
        };
        let Some(index) = by_process.or_else(by_target) else {
            return Origin {
                program: None,
                command: exe
                    .map(|exe| vec![exe.to_string_lossy().into_owned()])
                    .unwrap_or_default(),
                launch: None,
            };
        };
        let started = &mut self.started[index];
        started.claimed = true;
        started.launch.process_id = Some(process_id);
        Origin {
            program: started.launch.program.clone(),
            command: started.launch.command.clone(),
            launch: started.launch.launch,
        }
    }

    /// Forgets the claimed launches whose processes have no windows left.
    pub fn retain_processes(&mut self, processes: &HashSet<u32>) {
        self.started.retain(|started| {
            !started.claimed
                || started
                    .launch
                    .process_id
                    .is_some_and(|process| processes.contains(&process))
        });
    }

    pub fn len(&self) -> usize {
        self.started.len()
    }

    pub fn is_empty(&self) -> bool {
        self.started.is_empty()
    }
}

/// Windows paths compare without case and either slash.
pub fn same_path(a: &Path, b: &Path) -> bool {
    let fold = |path: &Path| path.to_string_lossy().replace('/', "\\").to_lowercase();
    fold(a) == fold(b)
}
