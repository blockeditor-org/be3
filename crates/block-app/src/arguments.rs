#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) dev_workspace: bool,
    pub(crate) desktop: bool,
    pub(crate) session: bool,
    pub(crate) install_session: Option<Option<String>>,
    pub(crate) server_address: Option<String>,
}

pub(crate) const LINUX: bool = cfg!(target_os = "linux");

pub(crate) fn parse(arguments: &[String], linux: bool) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    if arguments.first().map(String::as_str) == Some("--install-session") {
        if !linux {
            return Err(only_on_linux("--install-session"));
        }
        parsed.install_session = Some(arguments.get(1).cloned());
        return Ok(parsed);
    }
    for argument in arguments {
        match argument.as_str() {
            "--dev-workspace" => parsed.dev_workspace = true,
            "--desktop" => parsed.desktop = true,
            "--session" if linux => {
                parsed.session = true;
                parsed.desktop = true;
            }
            "--session" => return Err(only_on_linux("--session")),
            _ => match argument.strip_prefix("--server-address=") {
                Some(address) => parsed.server_address = Some(address.to_owned()),
                None => return Err(format!("unknown argument {argument}")),
            },
        }
    }
    Ok(parsed)
}

fn only_on_linux(flag: &str) -> String {
    format!(
        "{flag} makes the app a Linux login session, which only Linux has; --desktop runs the desktop shell in a window"
    )
}

#[cfg(test)]
mod tests;
