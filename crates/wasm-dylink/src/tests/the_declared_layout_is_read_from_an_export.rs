use super::*;

#[test]
fn the_declared_layout_is_read_from_an_export() {
    let declared = Declared::of(["memory", "__be3_dylink_100_20_3_2_16_4"]).unwrap();
    assert_eq!(
        declared,
        Declared {
            memory_size: 100,
            memory_align: 16,
            table_size: 3,
            table_align: 2,
            tls_size: 16,
            tls_align: 4,
        }
    );
    assert_eq!(Declared::of([declared.export().as_str()]), Ok(declared));
    assert!(Declared::of(["__be3_dylink_1_2_3_4"]).is_err());
    assert!(Declared::of(["memory"]).is_err());
}
