use super::*;

#[test]
fn a_path_is_escaped_in_its_uri() {
    assert_eq!(
        file_uri(Path::new("/run/user/1000/be-host/a shot#1é.png")),
        "file:///run/user/1000/be-host/a%20shot%231%C3%A9.png"
    );
}
