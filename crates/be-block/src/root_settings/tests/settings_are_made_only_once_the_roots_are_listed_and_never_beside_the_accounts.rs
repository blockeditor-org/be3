use super::*;

/// A reader whose listing of the roots may be from before they were listed (`stale`), as
/// be-host's cache is until it hears that the graph changed.
#[derive(Default)]
struct Listing {
    account: Uuid,
    /// The settings the account has at the root, once the roots are listed.
    existing: Option<Uuid>,
    listed: bool,
    stale: RefCell<Option<Vec<(Uuid, Uuid, Uuid)>>>,
    created: RefCell<Vec<Uuid>>,
}

impl SettingsGraph for Listing {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>> {
        if let Some(stale) = &*self.stale.borrow() {
            return Some(stale.clone());
        }
        self.listed.then(|| {
            self.existing
                .iter()
                .map(|block| (*block, Settings::CONTENT_TYPE, self.account))
                .collect()
        })
    }

    fn account(&self) -> Uuid {
        self.account
    }

    fn settings(&self, _: Uuid) -> Option<Settings> {
        Some(Settings::default())
    }

    fn settings_profile(&self, _: Uuid) -> Option<SettingsProfile> {
        None
    }

    fn create(&self, block: Uuid, _: Uuid, _: Vec<u8>, _: BlockParent, _: bool) -> Uuid {
        self.created.borrow_mut().push(block);
        block
    }

    fn edit_settings(&self, _: Uuid, _: Edit) {}

    fn edit_settings_profile(&self, _: Uuid, _: Edit) {}

    fn listed(&self) -> bool {
        if self.listed {
            *self.stale.borrow_mut() = None;
        }
        self.listed
    }
}

#[test]
fn settings_are_made_only_once_the_roots_are_listed_and_never_beside_the_accounts() {
    let unlisted = Listing::default();
    assert_eq!(RootSettings::default().ensure(&unlisted), None);
    assert!(
        unlisted.created.borrow().is_empty(),
        "nothing is made before the roots are listed"
    );

    let existing = Uuid::now_v7();
    let stale = Listing {
        existing: Some(existing),
        listed: true,
        stale: RefCell::new(Some(Vec::new())),
        ..Listing::default()
    };
    assert_eq!(
        RootSettings::default().ensure(&stale),
        Some(existing),
        "a listing read before the roots were listed is read again, and finds the settings"
    );
    assert!(
        stale.created.borrow().is_empty(),
        "no second settings is made beside the account's"
    );
}
