use std::path::PathBuf;

use crate::dbus::unix_path;

#[test]
fn the_socket_of_a_bus_address_is_its_unix_path() {
    assert_eq!(
        unix_path("unix:path=/run/user/1000/bus"),
        Some(PathBuf::from("/run/user/1000/bus"))
    );
    assert_eq!(
        unix_path("unix:guid=0123,path=/tmp/a%20bus/socket"),
        Some(PathBuf::from("/tmp/a bus/socket")),
        "options come in any order and bytes may be escaped"
    );
    assert_eq!(
        unix_path("tcp:host=localhost,port=4000;unix:path=/run/dbus/system_bus_socket"),
        Some(PathBuf::from("/run/dbus/system_bus_socket")),
        "the first unix address with a path is the one watched"
    );
    assert_eq!(unix_path("unix:abstract=/tmp/dbus-abc"), None);
    assert_eq!(unix_path("unix:path=/bad%2"), None);
}
