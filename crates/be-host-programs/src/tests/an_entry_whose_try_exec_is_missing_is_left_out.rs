use super::*;

#[test]
fn an_entry_whose_try_exec_is_missing_is_left_out() {
    let scratch = Scratch::new();
    scratch.executable("bin/present");
    scratch.write(
        "data/applications/present.desktop",
        application("Present", "present", "TryExec=present").as_bytes(),
    );
    scratch.write(
        "data/applications/absent.desktop",
        application("Absent", "absent", "TryExec=absent").as_bytes(),
    );
    let absolute = scratch.executable("opt/tool");
    scratch.write(
        "data/applications/tool.desktop",
        application("Tool", "tool", &format!("TryExec={}", absolute.display())).as_bytes(),
    );
    let programs = environment(&scratch, &["data"]).programs();
    assert_eq!(ids(&programs), ["present.desktop", "tool.desktop"]);
}
