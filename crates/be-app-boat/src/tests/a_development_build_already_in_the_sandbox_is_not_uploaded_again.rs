use super::*;

#[test]
fn a_development_build_already_in_the_sandbox_is_not_uploaded_again() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connect_terminal(Uuid::new_v4());
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Bytes(b"\x7fELF be".to_vec()));
    boat.ran("echo '", &said("present\n"));
    let placed = boat.ran("set -e", DONE);
    assert!(placed.contains(&format!("test -f be.part.{block}")));
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    boat.install_answered(block, true);
}
