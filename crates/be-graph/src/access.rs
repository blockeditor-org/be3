use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Access {
    #[default]
    None,
    KnowExists,
    View,
    Edit,
    Manage,
}

impl Access {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "No access",
            Self::KnowExists => "Knows it exists",
            Self::View => "Can view",
            Self::Edit => "Can edit",
            Self::Manage => "Can manage",
        }
    }

    pub fn can_know_exists(self) -> bool {
        self >= Self::KnowExists
    }

    pub fn can_view(self) -> bool {
        self >= Self::View
    }

    pub fn can_edit(self) -> bool {
        self >= Self::Edit
    }

    pub fn can_manage(self) -> bool {
        self >= Self::Manage
    }
}
