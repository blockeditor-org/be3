//! Whether the device runs on mains power, from UPower's `OnBattery` (org.freedesktop.UPower on
//! the system bus), for work that waits until it costs no battery (`be_host_stack::Rest`). With
//! no UPower nothing is reported: the device is taken to be on power.

use std::ops::ControlFlow;

use futures_util::StreamExt;
use zbus::Connection;
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::zvariant::Value;

use be_host_base::host::{self, Mailbox};

use crate::dbus::Bus;

const SERVICE: &str = "org.freedesktop.UPower";
const PATH: &str = "/org/freedesktop/UPower";
const PROPERTY: &str = "OnBattery";

/// Reports each change of whether the device is on mains power while it is kept.
pub struct Battery {
    _reports: Mailbox,
}

impl Battery {
    pub fn start(mut on_power: impl FnMut(bool) + 'static) -> Self {
        let (reports, post) = host::mailbox(move |powered: bool| on_power(powered));
        crate::dbus::spawn(async move {
            serve(&crate::dbus::SYSTEM, move |powered| post.send(powered)).await;
        });
        Self { _reports: reports }
    }
}

pub async fn serve(bus: &Bus, mut report: impl FnMut(bool) + Send + 'static) {
    loop {
        let connection = bus.connected().await;
        let serving = watch(&connection, &mut report);
        if crate::dbus::until_closed(&connection, serving)
            .await
            .is_break()
        {
            return;
        }
    }
}

/// Reads `OnBattery` and follows its changes until UPower goes; then waits for the connection.
async fn watch(connection: &Connection, report: &mut impl FnMut(bool)) -> ControlFlow<()> {
    let Ok(interface) = InterfaceName::try_from(SERVICE) else {
        return ControlFlow::Break(());
    };
    let proxy = async {
        PropertiesProxy::builder(connection)
            .destination(SERVICE)?
            .path(PATH)?
            .build()
            .await
    };
    let Ok(proxy) = proxy.await else {
        return ControlFlow::Continue(());
    };
    let Ok(mut changes) = proxy.receive_properties_changed().await else {
        return ControlFlow::Continue(());
    };
    let mut last = None;
    if let Ok(value) = proxy.get(interface.clone(), PROPERTY).await
        && let Value::Bool(on_battery) = &*value
    {
        last = Some(!on_battery);
        report(!on_battery);
    }
    while let Some(changed) = changes.next().await {
        let Ok(args) = changed.args() else {
            continue;
        };
        if args.interface_name != interface {
            continue;
        }
        let Some(Value::Bool(on_battery)) = args.changed_properties.get(PROPERTY) else {
            continue;
        };
        let powered = !on_battery;
        if last != Some(powered) {
            last = Some(powered);
            report(powered);
        }
    }
    ControlFlow::Continue(())
}
