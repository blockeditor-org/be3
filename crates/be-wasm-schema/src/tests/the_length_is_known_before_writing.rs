use super::*;

#[test]
fn the_length_is_known_before_writing() {
    let everything = person_v2();
    assert_eq!(encoded_len(&everything), Ok(bytes(&everything).len()));
    let tree = chain(20);
    assert_eq!(encoded_len(&tree), Ok(bytes(&tree).len()));
    assert_eq!(
        encoded_len(&Wrapper {
            inner: PersonV1 {
                name: String::new(),
                age: 0
            }
        }),
        Ok(1)
    );
}
