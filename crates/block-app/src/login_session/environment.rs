use std::collections::HashMap;

const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";
const SYSTEMD: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
const SYSTEMD_MANAGER: &str = "org.freedesktop.systemd1.Manager";

pub(crate) fn export(variables: Vec<(String, String)>) {
    if variables.is_empty() {
        return;
    }
    block_app_services::dbus::spawn(async move {
        let Some(bus) = block_app_services::dbus::session().await else {
            return;
        };
        let activation: HashMap<&str, &str> = variables
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        if let Err(error) = bus
            .call_method(
                Some(BUS),
                BUS_PATH,
                Some(BUS),
                "UpdateActivationEnvironment",
                &(activation,),
            )
            .await
        {
            eprintln!("block-app: D-Bus did not take the session's environment: {error}");
        }
        let assignments: Vec<String> = variables
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        if let Err(error) = bus
            .call_method(
                Some(SYSTEMD),
                SYSTEMD_PATH,
                Some(SYSTEMD_MANAGER),
                "SetEnvironment",
                &(assignments,),
            )
            .await
        {
            eprintln!("block-app: systemd did not take the session's environment: {error}");
        }
    });
}
