use std::collections::BTreeMap;

use crate::resolve::{resolve, stanzas};

#[test]
fn resolve_leaves_out_a_package_and_what_only_it_needs() {
    let index = "Package: gbm\nDepends: drm, gallium (= 1)\n\n\
                 Package: gallium\nDepends: llvm, drm\n\n\
                 Package: llvm\n\n\
                 Package: drm\n\n\
                 Package: nss\nDepends: llvm\n";
    let packages: BTreeMap<String, _> = stanzas(index)
        .into_iter()
        .map(|fields| (fields["Package"].clone(), fields))
        .collect();
    let names = |wanted: &[&str]| -> Vec<String> {
        resolve(&packages, wanted)
            .unwrap()
            .keys()
            .map(|name| (*name).to_owned())
            .collect()
    };
    assert_eq!(names(&["gbm"]), ["drm", "gallium", "gbm", "llvm"]);
    assert_eq!(names(&["gbm", "-gallium"]), ["drm", "gbm"]);
    // What another package needs is still kept.
    assert_eq!(
        names(&["gbm", "nss", "-gallium"]),
        ["drm", "gbm", "llvm", "nss"]
    );
    assert!(resolve(&packages, &["gbm", "-mesa"]).is_err());
}
