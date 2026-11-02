use super::*;

#[test]
fn introspection_says_whether_an_item_has_activate() {
    let item = |methods: &str| {
        format!(
            r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.DBus.Properties">
    <method name="Get"/>
  </interface>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Id" type="s" access="read"/>
    {methods}
  </interface>
</node>"#
        )
    };
    assert_eq!(
        introspected_activates(&item(
            r#"<method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>"#
        )),
        Some(true)
    );
    assert_eq!(
        introspected_activates(&item(
            r#"<method name="Scroll"/><method name="SecondaryActivate"/><method name="XAyatanaSecondaryActivate"/>"#
        )),
        Some(false),
        "libappindicator's items have no Activate"
    );
    assert_eq!(
        introspected_activates("<node><interface name='org.example.Other'/></node>"),
        None,
        "without the item's interface introspection says nothing"
    );
    assert_eq!(introspected_activates("not xml"), None);
}
