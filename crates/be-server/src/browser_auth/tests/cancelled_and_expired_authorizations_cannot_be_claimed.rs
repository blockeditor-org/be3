use super::*;

#[tokio::test]
async fn cancelled_and_expired_authorizations_cannot_be_claimed() {
    let sessions = Arc::new(Sessions::default());
    let (first, first_secret) = sessions.begin(1).unwrap();
    let (second, second_secret) = sessions.begin(1).unwrap();
    assert!(sessions.finish(1, &first, &first_secret).is_err());
    assert!(matches!(
        sessions.finish(2, &second, &second_secret),
        Ok(Finished::Waiting(_))
    ));
    let (expired, expired_secret) = sessions.begin(1).unwrap();
    {
        let mut entries = sessions.pending.lock().unwrap();
        let entry = entries.get_mut(&expired).unwrap();
        entry.account = Some(Uuid::new_v4());
        entry.expires -= LIFETIME + Duration::from_secs(1);
    }
    assert!(sessions.finish(1, &expired, &expired_secret).is_err());
    let form = sessions.form("").unwrap();
    sessions.forms.lock().unwrap().get_mut(&form).unwrap().0 -= LIFETIME + Duration::from_secs(1);
    assert!(sessions.consume_form(&form, "").is_err());
}
