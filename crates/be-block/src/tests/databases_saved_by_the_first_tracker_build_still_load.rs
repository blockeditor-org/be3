use be_model::Document;
use be_model::schema::check_frozen;

use super::{frozen_database, frozen_database_schema, frozen_database_view};
use crate::database::Database;
use crate::database_schema::{ChoiceColor, ChoiceIcon, DatabaseSchema};
use crate::database_view::DatabaseView;

fn bytes(hex: &str) -> Vec<u8> {
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn databases_saved_by_the_first_tracker_build_still_load() {
    assert_eq!(
        check_frozen::<Database>(frozen_database::REGISTRY, frozen_database::DOCUMENT),
        Ok(())
    );
    assert_eq!(
        check_frozen::<DatabaseSchema>(
            frozen_database_schema::REGISTRY,
            frozen_database_schema::DOCUMENT
        ),
        Ok(())
    );
    assert_eq!(
        check_frozen::<DatabaseView>(
            frozen_database_view::REGISTRY,
            frozen_database_view::DOCUMENT
        ),
        Ok(())
    );

    let schema =
        Document::<DatabaseSchema>::from_bytes(&bytes(frozen_database_schema::DOCUMENT)).unwrap();
    let status = schema.root().field_named("Status").unwrap();
    let names: Vec<_> = status
        .enum_options
        .iter()
        .map(|option| &option.name)
        .collect();
    assert_eq!(names, ["To do", "Done"]);
    assert!(status.enum_options.iter().all(|option| {
        option.color == ChoiceColor::Gray && option.icon == ChoiceIcon::None && !option.closed
    }));
    assert_eq!(status.default_option, None);

    let view =
        Document::<DatabaseView>::from_bytes(&bytes(frozen_database_view::DOCUMENT)).unwrap();
    assert_eq!(view.root().group_by, Some(status.id));
    assert_eq!(view.root().filters.len(), 6);
    assert!(view.root().hidden_fields.is_empty());
}
