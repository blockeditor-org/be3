use super::*;

/// Filling the server's pending sign-ins or forms never refuses a newcomer: the oldest entry is
/// let go of.
#[tokio::test]
async fn a_full_table_lets_go_of_its_oldest() {
    let mut table = Bounded::new(4, 2);
    for (index, last) in [1u8, 2, 3, 4, 5, 6].into_iter().enumerate() {
        table.insert(index.to_string(), index, IpAddr::from([192, 0, 2, last]));
    }
    assert_eq!(table.len(), 4);
    assert!(table.get("0").is_none() && table.get("1").is_none());
    assert_eq!(table.get("5"), Some(&5), "the newest entry was refused");
    let busy = IpAddr::from([198, 51, 100, 1]);
    table.insert("a".into(), 10, busy);
    table.insert("b".into(), 11, busy);
    table.insert("c".into(), 12, busy);
    assert!(
        table.get("a").is_none() && table.get("c").is_some(),
        "an address held more than it may"
    );
    assert_eq!(table.len(), 4);
}
