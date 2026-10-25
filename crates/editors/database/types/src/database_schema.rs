use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::Root;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum DatabaseFieldType {
    #[default]
    String,
    Number,
    Enum,
    Block,
    Boolean,
    Color,
    Datetime,
    Relation,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum DatabaseNumberScale {
    #[default]
    Linear,
    Logarithmic,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct DatabaseNumberOptions {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
    pub scale: DatabaseNumberScale,
}

impl DatabaseNumberOptions {
    pub fn effective_step(self) -> f64 {
        self.step.unwrap_or(match self.scale {
            DatabaseNumberScale::Linear => 1.0,
            DatabaseNumberScale::Logarithmic => 1.01,
        })
    }

    pub fn normalized(mut self) -> Self {
        self.minimum = self.minimum.filter(|value| value.is_finite());
        self.maximum = self.maximum.filter(|value| value.is_finite());
        self.step = self.step.filter(|value| value.is_finite());
        if self.scale == DatabaseNumberScale::Logarithmic {
            self.minimum = self.minimum.filter(|value| *value > 0.0);
            self.maximum = self.maximum.filter(|value| *value > 0.0);
        }
        if let (Some(minimum), Some(maximum)) = (self.minimum, self.maximum)
            && minimum > maximum
        {
            self.minimum = Some(maximum);
            self.maximum = Some(minimum);
        }
        self.step = match self.scale {
            DatabaseNumberScale::Linear => self.step.filter(|value| *value > 0.0),
            DatabaseNumberScale::Logarithmic => self.step.filter(|value| *value > 1.0),
        };
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
pub struct DatabaseBlockOptions {
    pub block_type: Option<Uuid>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
pub struct DatabaseRelationOptions {
    #[serde(with = "be_model::references::block_ref::option")]
    pub database: Option<Uuid>,
    pub multiple: bool,
    pub inverse: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum ChoiceColor {
    #[default]
    Gray,
    Blue,
    Teal,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
}

impl ChoiceColor {
    pub const ALL: [Self; 9] = [
        Self::Gray,
        Self::Blue,
        Self::Teal,
        Self::Green,
        Self::Yellow,
        Self::Orange,
        Self::Red,
        Self::Pink,
        Self::Purple,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Gray => "gray",
            Self::Blue => "blue",
            Self::Teal => "teal",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
            Self::Pink => "pink",
            Self::Purple => "purple",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|color| color.name().eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum ChoiceIcon {
    #[default]
    None,
    Backlog,
    Todo,
    Started,
    Review,
    Done,
    Canceled,
    Low,
    Medium,
    High,
    Urgent,
    Flag,
    Star,
}

impl ChoiceIcon {
    pub const ALL: [Self; 13] = [
        Self::None,
        Self::Backlog,
        Self::Todo,
        Self::Started,
        Self::Review,
        Self::Done,
        Self::Canceled,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Urgent,
        Self::Flag,
        Self::Star,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::Started => "started",
            Self::Review => "review",
            Self::Done => "done",
            Self::Canceled => "canceled",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Urgent => "urgent",
            Self::Flag => "flag",
            Self::Star => "star",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|icon| icon.name().eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DatabaseEnumOption {
    pub id: Uuid,
    pub name: String,
    pub color: ChoiceColor,
    pub icon: ChoiceIcon,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DatabaseField {
    pub id: Uuid,
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub enum_options: Vec<DatabaseEnumOption>,
    pub number_options: DatabaseNumberOptions,
    pub block_options: DatabaseBlockOptions,
    pub relation_options: DatabaseRelationOptions,
    pub default_option: Option<Uuid>,
}

impl DatabaseField {
    pub fn option(&self, id: Uuid) -> Option<&DatabaseEnumOption> {
        self.enum_options.iter().find(|option| option.id == id)
    }

    pub fn closes(&self, id: Uuid) -> bool {
        self.option(id).is_some_and(|option| option.closed)
    }

    pub fn first_closed(&self) -> Option<&DatabaseEnumOption> {
        self.enum_options.iter().find(|option| option.closed)
    }

    pub fn default_value(&self) -> Option<Uuid> {
        self.default_option.filter(|id| self.option(*id).is_some())
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseSchema {
    pub fields: List<SchemaField>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct SchemaField {
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub enum_options: List<EnumOption>,
    pub number_options: DatabaseNumberOptions,
    pub block_options: DatabaseBlockOptions,
    pub relation_options: DatabaseRelationOptions,
    pub default_option: Option<Uuid>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct EnumOption {
    pub name: String,
    pub color: ChoiceColor,
    pub icon: ChoiceIcon,
    pub closed: bool,
}

impl DatabaseSchema {
    pub fn fields(&self) -> Vec<DatabaseField> {
        self.fields
            .iter()
            .map(|field| DatabaseField {
                id: field.id.as_uuid(),
                name: field.name.clone(),
                field_type: field.field_type,
                enum_options: field
                    .enum_options
                    .iter()
                    .map(|option| DatabaseEnumOption {
                        id: option.id.as_uuid(),
                        name: option.name.clone(),
                        color: option.color,
                        icon: option.icon,
                        closed: option.closed,
                    })
                    .collect(),
                number_options: field.number_options,
                block_options: field.block_options,
                relation_options: field.relation_options.clone(),
                default_option: field.default_option,
            })
            .collect()
    }

    pub fn title_field(&self) -> Option<Uuid> {
        self.fields
            .iter()
            .find(|field| field.field_type == DatabaseFieldType::String)
            .map(|field| field.id.as_uuid())
    }

    pub fn field_named(&self, name: &str) -> Option<DatabaseField> {
        let name = name.trim();
        self.fields()
            .into_iter()
            .find(|field| field.name.trim().eq_ignore_ascii_case(name))
    }

    pub fn add_field(name: impl Into<String>, field_type: DatabaseFieldType) -> (Uuid, Edit) {
        let field = SchemaField {
            name: name.into(),
            field_type,
            ..SchemaField::default()
        };
        let (id, change) = Self::FIELDS.insert(ObjectId::ROOT, Anchor::End, &field);
        (id.as_uuid(), change.into())
    }

    pub fn remove_field(field: Uuid) -> Edit {
        Change::remove(ObjectId::from_uuid(field)).into()
    }

    pub fn rename_field(field: Uuid, name: impl Into<String>) -> Edit {
        SchemaField::NAME
            .set(ObjectId::from_uuid(field), &name.into())
            .into()
    }

    pub fn set_field_type(field: Uuid, field_type: DatabaseFieldType) -> Edit {
        SchemaField::FIELD_TYPE
            .set(ObjectId::from_uuid(field), &field_type)
            .into()
    }

    pub fn set_number_options(field: Uuid, options: DatabaseNumberOptions) -> Edit {
        SchemaField::NUMBER_OPTIONS
            .set(ObjectId::from_uuid(field), &options.normalized())
            .into()
    }

    pub fn set_block_options(field: Uuid, options: DatabaseBlockOptions) -> Edit {
        SchemaField::BLOCK_OPTIONS
            .set(ObjectId::from_uuid(field), &options)
            .into()
    }

    pub fn set_relation_options(field: Uuid, options: DatabaseRelationOptions) -> Edit {
        SchemaField::RELATION_OPTIONS
            .set(ObjectId::from_uuid(field), &options)
            .into()
    }

    pub fn add_enum_option(field: Uuid, name: impl Into<String>) -> (Uuid, Edit) {
        let option = EnumOption {
            name: name.into(),
            ..EnumOption::default()
        };
        let (id, change) =
            SchemaField::ENUM_OPTIONS.insert(ObjectId::from_uuid(field), Anchor::End, &option);
        (id.as_uuid(), change.into())
    }

    pub fn rename_enum_option(option: Uuid, name: impl Into<String>) -> Edit {
        EnumOption::NAME
            .set(ObjectId::from_uuid(option), &name.into())
            .into()
    }

    pub fn set_option_color(option: Uuid, color: ChoiceColor) -> Edit {
        EnumOption::COLOR
            .set(ObjectId::from_uuid(option), &color)
            .into()
    }

    pub fn set_option_icon(option: Uuid, icon: ChoiceIcon) -> Edit {
        EnumOption::ICON
            .set(ObjectId::from_uuid(option), &icon)
            .into()
    }

    pub fn set_option_closed(option: Uuid, closed: bool) -> Edit {
        EnumOption::CLOSED
            .set(ObjectId::from_uuid(option), &closed)
            .into()
    }

    pub fn set_default_option(field: Uuid, option: Option<Uuid>) -> Edit {
        SchemaField::DEFAULT_OPTION
            .set(ObjectId::from_uuid(field), &option)
            .into()
    }

    pub fn move_field(field: Uuid, after: Option<Uuid>) -> Edit {
        let anchor = after.map_or(Anchor::Start, |after| {
            Anchor::After(ObjectId::from_uuid(after))
        });
        Self::FIELDS
            .move_into(ObjectId::ROOT, anchor, ObjectId::from_uuid(field))
            .into()
    }

    pub fn move_enum_option(field: Uuid, option: Uuid, after: Option<Uuid>) -> Edit {
        let anchor = after.map_or(Anchor::Start, |after| {
            Anchor::After(ObjectId::from_uuid(after))
        });
        SchemaField::ENUM_OPTIONS
            .move_into(
                ObjectId::from_uuid(field),
                anchor,
                ObjectId::from_uuid(option),
            )
            .into()
    }

    pub fn defaults(&self) -> Vec<(Uuid, crate::database::DatabaseValue)> {
        self.fields()
            .into_iter()
            .filter(|field| field.field_type == DatabaseFieldType::Enum)
            .filter_map(|field| {
                Some((
                    field.id,
                    crate::database::DatabaseValue::Enum(field.default_value()?),
                ))
            })
            .collect()
    }

    pub fn remove_enum_option(option: Uuid) -> Edit {
        Change::remove(ObjectId::from_uuid(option)).into()
    }
}

impl Root for DatabaseSchema {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6461_7461_6261_7365_2d73_6368_656d_6101);

    const FROZEN: bool = true;
}

pub type DatabaseSchemaContent = Document<DatabaseSchema>;
