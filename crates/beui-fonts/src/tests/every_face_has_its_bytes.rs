use super::*;

#[test]
fn every_face_has_its_bytes() {
    for face in FACES {
        let bytes = bytes(face.file).unwrap_or_else(|| panic!("{} has no bytes", face.file));
        assert_eq!(&bytes[..4], [0, 1, 0, 0], "{} is not a TrueType font", face.file);
    }
}
