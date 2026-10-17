use super::*;

#[test]
fn the_embedded_server_listens_where_asked() {
    assert_eq!(
        parse(
            &words("--dev-workspace --server-address=127.0.0.1:4100"),
            true
        ),
        Ok(Arguments {
            dev_workspace: true,
            server_address: Some("127.0.0.1:4100".to_owned()),
            ..Arguments::default()
        })
    );
}
