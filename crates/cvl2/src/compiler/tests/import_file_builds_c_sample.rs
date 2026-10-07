use super::*;

#[test]
fn import_file_builds_c_sample() {
    let artifact = import_file("c.qxc", include_str!("../../../samples/c.qxc"))
        .unwrap_or_else(|errors| panic!("c.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(name, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file, got {folder:?}");
    };
    assert_eq!(name, "lib.c");
    assert_eq!(
        String::from_utf8(file.value.clone()).unwrap(),
        "int transmogrify(int _a0, int _a1);
static int cvl2_fn_0(void);

int transmogrify(int _a0, int _a1) {
    int _3;
    {
        int _4 = _a0 == _a1;
        if (_4) {
            int _6 = cvl2_fn_0();
            _3 = _6;
            goto _l3;
        }
        int _9 = _a0 + _a1;
        _3 = _9;
    }
    _l3:;
    return _3;
}

static int cvl2_fn_0(void) {
    return 0;
}
"
    );
}
