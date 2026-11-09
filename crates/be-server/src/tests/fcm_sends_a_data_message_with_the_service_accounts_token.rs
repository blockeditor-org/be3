use super::*;
use base64::Engine;
use be_block_protocol::{PushMessage, PushService};
use ring::signature::{KeyPair, RSA_PKCS1_2048_8192_SHA256, RsaKeyPair, UnparsedPublicKey};

/// A service account's private key made for this test alone; no Firebase project knows it.
const TEST_SERVICE_ACCOUNT_KEY: &str = "\
-----BEGIN PRIVATE KEY-----\n\
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQD1Ug1fi7JaAEdD\n\
zDEizysitZ6seyCrHHe3imwO2BZrLzSIdPGU2ovU9dZEAhWoaSHoFd+cAE65VmQz\n\
DuevmFQnsUAzKtFC95qQRDmFYSESIqmzT0Rmrh/Fh9dfzD3Ro9f98iWkjj2qTjQ/\n\
2FOHWcRGudbf9BNprX3WOhxyiQWuxzCzkE5bcTGj4iOaJAgGymZnwhSvFn2o6rBB\n\
vJuO1Pj4BbdCWzwVmTTtOypUcsbnQDHI01YxGtu+rKLGUgl3+0638ROqHYeBXApr\n\
v3fB0zruV779PfgQbZTFjVwR3uFKlFqlrDQq1LY+mbSQdIC01hNZ+qvclFU2YcDH\n\
YqpGF9oPAgMBAAECggEACGCokUF0MigTDhAMV/Z5E7D6b5nLuDGJg/c9AnLDdFdC\n\
9Q8dT1UyLO//miSehpyCuKL8Xiz1LfIdmcHh03+vf5Zlu5YJWjDTBGCNZ25Di5O5\n\
WU9qSWh+92AX0L1o/QkohNppe60vLyP5G14pyds5TzPk7K8T9YaHfE808ZpeokFV\n\
6wxWCbIkg/Fry8HXD5r2gontZDNkGDRQxR3D85qM3+Nj1XDYfgLycGMj+RMLD26b\n\
Q4AU7wC8vZdM7F6umIOmuWs6Iy24uEoWV6XhB8a1A7y2JiwWEKLc5s0HeFXcaTeR\n\
S53s4J0+cNgjNayOmPJ6su4+8BrinKR6/ATpiKDeKQKBgQD9QS+p8SB/ZEAlbrQv\n\
lG4BWo3JeNl4gpncHcnRpa5eTQ1WQ2JfwadwxbHOhUUGLBSwvr8zaUp+PpTGLWuk\n\
o4ZXouuMCDckPmzb1yE7BmUKU2nfGiSAFETvOL/6uQLiUzvH85gKyhq5gq6hfV8Z\n\
8b7e6+kBiGSCJVa9bUGY5ivuWQKBgQD3+tkN7Op4F9JEjAG7i5sB4SxYfcXfmFYK\n\
4AZqw4SWLp2903j6wgjBxXR8lNjM29iNc1zAooYgft/wQiO9bI+J60Uw+93/l722\n\
5OqeomJ9RMI7CjLXZRB+kgpFoW44woPpKyytxiJOchlk7qWaftHGhCoR3uYcPO/v\n\
DhzzR8qOpwKBgAxohDhUWx5CwC3VVbi7EgS49hd391fDNlyljPzbnEZ8ifm/BXPh\n\
nh2VYhfXScF6qWzbe6y+LOswj8OB+DMx2+QxVE8hG+qHsHe4+6TTtGGkIpzOwBkS\n\
ZSo/YpCbPfvuzhAG7F0i99o8/+djqCQyRwLPFjSABUSnJa/USZF/PYnJAoGAarj1\n\
oy2cNzUhVEjhEgLiIivzdt4iQ7l/Bk0l1drxe+TyFFknztulAyfkmx1ANOJEz5Of\n\
jnQjv2Z7IB6G3SYUnGm5rHwpLa/zgPwCEWWXd4wY/2nDOboQIDjf3Nm94wWr2bSw\n\
FNWZvCMU1MB0WwpLY+FLbcd9RxfetwZe5UC3Zy8CgYEAmjLIID+cFmv0/3pjChDo\n\
JZ1F0xcF/ArbDfxFFaPA1D3IPVN51Jiwdndr42nEnL0qIAeTFb1FAtU0mSAq4UoL\n\
/nNsm/j5miBtVBF8SbP4NNKhMWZuwO+gTDdYM2qQd4Fxb1JYoYVMa9caRcX9qkcC\n\
QdwQB1uWF7DA7nA1aBhoiXg=\n\
-----END PRIVATE KEY-----\n\
";

#[tokio::test]
async fn fcm_sends_a_data_message_with_the_service_accounts_token() {
    let mut service = TestPushService::start(|path| match path {
        "/token" => (
            200,
            r#"{"access_token":"an-access-token","expires_in":3600}"#.to_owned(),
        ),
        _ => (
            200,
            r#"{"name":"projects/a-project/messages/1"}"#.to_owned(),
        ),
    })
    .await;
    let account = ureq::serde_json::json!({
        "type": "service_account",
        "project_id": "a-project",
        "client_email": "pusher@a-project.iam.gserviceaccount.com",
        "private_key": TEST_SERVICE_ACCOUNT_KEY,
        "token_uri": format!("{}/token", service.url),
    });
    let mut fcm = push::FcmConfig::from_service_account(&account.to_string()).unwrap();
    fcm.endpoint = service.url.clone();
    let public = RsaKeyPair::from_pkcs8(&fcm.private_key)
        .unwrap()
        .public_key()
        .as_ref()
        .to_vec();
    let harness = Harness::configured(ServerConfig {
        push: push::PushConfig {
            allow_local_endpoints: false,
            fcm: Some(fcm),
        },
        ..ServerConfig::OPEN
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut member, _) = viewer_of(&harness, &mut owner, block, "member@example.com").await;
    let offers = member
        .call(|peer| client::notifications::offers_fcm(peer))
        .await;
    assert!(matches!(offers, Ok(true)), "{offers:?}");
    let device = TestDevice::new();
    member
        .register_endpoint(device.endpoint(PushService::Fcm, "a-registration-token".into()))
        .await;
    member.sign_out().await;

    owner.notify(block, b"sealed", Vec::new()).await;
    let base64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let asked = service.next().await;
    assert_eq!(asked.path, "/token");
    let form = String::from_utf8(asked.body).unwrap();
    let assertion = form
        .split('&')
        .find_map(|field| field.strip_prefix("assertion="))
        .expect("the token request carries an assertion");
    assert!(
        form.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer"),
        "{form}"
    );
    let (signed, signature) = assertion.rsplit_once('.').unwrap();
    UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, &public)
        .verify(signed.as_bytes(), &base64.decode(signature).unwrap())
        .expect("the service account signed the assertion");
    let claims = base64.decode(signed.split('.').nth(1).unwrap()).unwrap();
    let claims: ureq::serde_json::Value = ureq::serde_json::from_slice(&claims).unwrap();
    assert_eq!(claims["iss"], "pusher@a-project.iam.gserviceaccount.com");
    assert_eq!(claims["aud"], format!("{}/token", service.url));
    assert_eq!(
        claims["scope"],
        "https://www.googleapis.com/auth/firebase.messaging"
    );

    let sent = service.next().await;
    assert_eq!(sent.path, "/v1/projects/a-project/messages:send");
    assert_eq!(sent.headers["authorization"], "Bearer an-access-token");
    let message: ureq::serde_json::Value = ureq::serde_json::from_slice(&sent.body).unwrap();
    assert_eq!(message["message"]["token"], "a-registration-token");
    assert_eq!(message["message"]["android"]["priority"], "high");
    let body = base64
        .decode(message["message"]["data"]["push"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        device.open(&body),
        PushMessage {
            organization,
            block,
            sealed: b"sealed".to_vec(),
        }
    );

    harness.stop().await;
}
