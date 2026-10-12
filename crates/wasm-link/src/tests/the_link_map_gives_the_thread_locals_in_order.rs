use super::*;

#[test]
fn the_link_map_gives_the_thread_locals_in_order() {
    let found = map::thread_locals(MAP).unwrap();
    assert_eq!(
        found.inputs,
        ["libsdk.rlib(sdk.o):(.tdata.LOCAL)", "libc.a(errno.o):(.tbss.errno)"]
    );
    assert_eq!(
        found.symbols,
        [
            map::Placed {
                name: "LOCAL".to_owned(),
                offset: 0,
                size: 4
            },
            map::Placed {
                name: "errno".to_owned(),
                offset: 4,
                size: 8
            }
        ]
    );
}
