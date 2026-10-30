#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) dev_workspace: bool,
    pub(crate) desktop: bool,
    pub(crate) drm: bool,
    pub(crate) install_login_session: Option<Option<String>>,
}

pub(crate) const LINUX: bool = cfg!(target_os = "linux");

pub(crate) const DRM: &str = "drm";

pub(crate) fn parse(
    arguments: &[String],
    platform: Option<&str>,
    linux: bool,
) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    if arguments.first().map(String::as_str) == Some("--install-login-session") {
        if !linux {
            return Err(
                "--install-login-session installs the app as a Linux login session, which only Linux has; --desktop runs the desktop shell in a window".to_owned(),
            );
        }
        parsed.install_login_session = Some(arguments.get(1).cloned());
        return Ok(parsed);
    }
    for argument in arguments {
        match argument.as_str() {
            "--dev-workspace" => parsed.dev_workspace = true,
            "--desktop" => parsed.desktop = true,
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    if platform == Some(DRM) {
        if !linux {
            return Err(format!(
                "{}={DRM} runs the app on the displays and input devices themselves, which only Linux can; --desktop runs the desktop shell in a window",
                beui_host::native::environment::PLATFORM
            ));
        }
        parsed.drm = true;
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;
