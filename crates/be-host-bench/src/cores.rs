use std::os::unix::process::CommandExt;
use std::process::Command;

pub struct Cores {
    server: Option<usize>,
    app: Vec<usize>,
}

impl Cores {
    pub fn split() -> Result<Self, String> {
        let allowed = allowed()?;
        Ok(match allowed.split_last() {
            Some((server, app)) if !app.is_empty() => Self {
                server: Some(*server),
                app: app.to_vec(),
            },
            _ => Self {
                server: None,
                app: allowed,
            },
        })
    }

    pub fn describe(&self) -> String {
        match self.server {
            Some(server) => format!(
                "the server and its network on core {server}, block-app and the bench on cores {:?}",
                self.app
            ),
            None => "one core, shared by the server, block-app and the bench".to_owned(),
        }
    }

    pub fn keep_to_app(&self) -> Result<(), String> {
        match self.server {
            Some(_) => {
                pin(&self.app).map_err(|error| format!("could not pin to {:?}: {error}", self.app))
            }
            None => Ok(()),
        }
    }

    pub fn keep_to_server(&self, command: &mut Command) {
        let Some(server) = self.server else {
            return;
        };
        // unsafe(ffi)
        unsafe {
            command.pre_exec(move || pin(&[server]));
        }
    }
}

fn allowed() -> Result<Vec<usize>, String> {
    // unsafe(ffi)
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return Err(format!(
                "could not read which cores the bench may use: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok((0..libc::CPU_SETSIZE as usize)
            .filter(|core| libc::CPU_ISSET(*core, &set))
            .collect())
    }
}

fn pin(cores: &[usize]) -> std::io::Result<()> {
    // unsafe(ffi)
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for core in cores {
            libc::CPU_SET(*core, &mut set);
        }
        match libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) {
            0 => Ok(()),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
}
