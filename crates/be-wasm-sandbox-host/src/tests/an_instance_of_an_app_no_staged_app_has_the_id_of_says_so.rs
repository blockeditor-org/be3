use super::*;

#[test]
fn an_instance_of_an_app_no_staged_app_has_the_id_of_says_so() {
    let mut pump = Pump::new();
    let (sent, next) = pump.run(async {
        let instance =
            guest::sandbox::Instance::new(Source::App("be3.nowhere".to_owned()), app_config());
        (
            instance.send(vec![b"hello".to_vec()]).await,
            instance.next().await.unwrap(),
        )
    });
    assert!(sent.is_ok(), "a send to it is taken and dropped");
    assert_eq!(next, Err("no staged app has the id be3.nowhere".to_owned()));
}
