use super::*;

mod a_path_is_escaped_in_its_uri;
mod a_screenshot_is_saved_and_its_uri_answered;
mod no_picture_fails_the_request;

use crate::dbus::test_bus::{PrivateBus, owned, within};

fn serving(bus: &PrivateBus, picture: Option<Image>, directory: PathBuf) {
    let ask: Ask = Arc::new(move |answer: Answer| {
        let _ = answer.send(picture.clone());
    });
    let shared = bus.shared();
    crate::dbus::spawn(async move {
        serve(&shared, ask, directory).await;
    });
}

async fn screenshot(client: &Connection) -> (u32, HashMap<String, OwnedValue>) {
    let handle = OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/1_1/t")
        .expect("a handle");
    let options: HashMap<String, OwnedValue> =
        HashMap::from([("interactive".to_owned(), OwnedValue::from(false))]);
    within(
        "the portal answers Screenshot",
        client.call_method(
            Some(NAME),
            PATH,
            Some("org.freedesktop.impl.portal.Screenshot"),
            "Screenshot",
            &(handle, "", "wayland:", options),
        ),
    )
    .await
    .expect("the portal takes the request")
    .body()
    .deserialize()
    .expect("a response and results")
}
