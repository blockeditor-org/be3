use crate::test_client::{TestClient, server};

mod the_primary_selection_is_advertised_whether_or_not_it_is_switched_on;

fn advertised(client: &TestClient, interface: &str) -> usize {
    client
        .received
        .globals
        .iter()
        .filter(|(_, name, _)| name == interface)
        .count()
}
