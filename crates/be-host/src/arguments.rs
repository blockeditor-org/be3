#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) dev_workspace: bool,
    /// `--dev-workspace=URL`: the server the dev workspace opens on, instead of the embedded
    /// one. It signs in with the token in `BE_DEV_TOKEN` when that is set, and through the
    /// browser otherwise, as the web build's `?dev-workspace` does.
    pub(crate) dev_server: Option<String>,
    pub(crate) desktop: bool,
    pub(crate) drm: bool,
    pub(crate) login_session: Option<LoginSessionSetup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoginSessionSetup {
    Install(Option<String>),
    Uninstall(Option<String>),
    ApplyChanges,
}

pub(crate) const INSTALL: &str = "--install-login-session";
pub(crate) const UNINSTALL: &str = "--uninstall-login-session";
pub(crate) const APPLY_CHANGES: &str = "--apply-login-session-changes";

pub(crate) const LINUX: bool = cfg!(target_os = "linux");

pub(crate) const DRM: &str = "drm";

pub(crate) fn parse(
    arguments: &[String],
    platform: Option<&str>,
    linux: bool,
) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    let prefix = arguments.get(1).cloned();
    let setup = match arguments.first().map(String::as_str) {
        Some(INSTALL) => Some(LoginSessionSetup::Install(prefix)),
        Some(UNINSTALL) => Some(LoginSessionSetup::Uninstall(prefix)),
        Some(APPLY_CHANGES) => Some(LoginSessionSetup::ApplyChanges),
        _ => None,
    };
    if let Some(setup) = setup {
        if !linux {
            return Err(format!(
                "{} sets be-host up as a Linux login session, which only Linux has; --desktop runs the desktop shell in a window",
                arguments[0]
            ));
        }
        parsed.login_session = Some(setup);
        return Ok(parsed);
    }
    for argument in arguments {
        match argument.as_str() {
            "--dev-workspace" => parsed.dev_workspace = true,
            other if other.starts_with("--dev-workspace=") => {
                let url = &other["--dev-workspace=".len()..];
                if url.is_empty() {
                    return Err("--dev-workspace=URL needs the server's address".to_owned());
                }
                parsed.dev_workspace = true;
                parsed.dev_server = Some(url.to_owned());
            }
            "--desktop" => parsed.desktop = true,
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    if platform == Some(DRM) {
        if !linux {
            return Err(format!(
                "{}={DRM} runs be-host on the displays and input devices themselves, which only Linux can; --desktop runs the desktop shell in a window",
                beui_host::native::environment::PLATFORM
            ));
        }
        parsed.drm = true;
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;
