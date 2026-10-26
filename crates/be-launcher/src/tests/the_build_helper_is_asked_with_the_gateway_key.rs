use super::*;

#[test]
fn the_build_helper_is_asked_with_the_gateway_key() {
    let commit = "c".repeat(40);
    let product = Product::App;
    let base = format!(
        "{}/products/native-{}/{commit}",
        helper::HELPER,
        helper::platform()
    );
    let mut store = Store::default();
    store.answer(
        &base,
        200,
        r#"{"product": "native", "state": "queued", "position": 2}"#,
    );
    store.answer(&format!("{base}/log?from=5"), 200, "built");

    let mut asking = Helper {
        http: &mut store,
        key: "the key".to_owned(),
    };
    let status = asking.request(product, &commit).unwrap();
    assert_eq!(status.state, helper::State::Queued);
    assert_eq!(status.position, Some(2));
    assert_eq!(asking.log(product, &commit, 5).unwrap(), b"built");
    let asked = &store.asked[0];
    assert_eq!(
        (asked.method.as_str(), asked.url.as_str()),
        ("POST", base.as_str())
    );
    assert_eq!(
        &asked.headers,
        &vec![("Be3-Gateway-Authorization".to_owned(), "the key".to_owned())]
    );

    store.answer(&base, 401, "no");
    let mut refused = Helper {
        http: &mut store,
        key: "wrong".to_owned(),
    };
    assert_eq!(
        refused.status(product, &commit).unwrap_err(),
        helper::UNAUTHORIZED
    );
}
