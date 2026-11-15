use super::*;

#[test]
fn moving_after_a_neighbour_names_the_place_by_the_row_before() {
    let mut order = vec!['a', 'b', 'c', 'd'];
    move_after(&mut order, 'a', Some('c'));
    assert_eq!(order, vec!['b', 'c', 'a', 'd']);
    move_after(&mut order, 'd', None);
    assert_eq!(order, vec!['d', 'b', 'c', 'a'], "after nothing is the front");
    move_after(&mut order, 'e', Some('b'));
    assert_eq!(order, vec!['d', 'b', 'e', 'c', 'a'], "an item not there is added");
    move_after(&mut order, 'c', Some('c'));
    move_after(&mut order, 'c', Some('z'));
    assert_eq!(
        order,
        vec!['d', 'b', 'e', 'c', 'a'],
        "after itself, or after a row not shown, it stays"
    );
}
