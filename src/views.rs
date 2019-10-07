use crate::models;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Kit {
    pub id: i32,
    pub serial: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub privacy_public_dashboard: bool,
    pub privacy_show_on_map: bool,
}

impl From<models::Kit> for Kit {
    fn from(kit: models::Kit) -> Self {
        use bigdecimal::ToPrimitive;

        let models::Kit {
            id,
            serial,
            name,
            description,
            latitude,
            longitude,
            privacy_public_dashboard,
            privacy_show_on_map,
            ..
        } = kit;
        Self {
            id,
            serial,
            name,
            description,
            latitude: latitude.and_then(|l| l.to_f64()),
            longitude: longitude.and_then(|l| l.to_f64()),
            privacy_public_dashboard,
            privacy_show_on_map,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FullUser {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub email_address: String,
    pub use_email_address_for_gravatar: bool,
    pub gravatar_alternative: String,
}

impl From<models::User> for FullUser {
    fn from(user: models::User) -> Self {
        let models::User {
            id,
            username,
            display_name,
            email_address,
            use_email_address_for_gravatar,
            gravatar_alternative,
            ..
        } = user;
        Self {
            id,
            username,
            display_name,
            email_address,
            use_email_address_for_gravatar,
            gravatar_alternative,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub display_name: String,
    pub gravatar: String,
}

impl From<models::User> for User {
    fn from(user: models::User) -> Self {
        let models::User {
            username,
            display_name,
            email_address,
            use_email_address_for_gravatar,
            gravatar_alternative,
            ..
        } = user;
        Self {
            username,
            display_name,
            gravatar: if use_email_address_for_gravatar {
                email_address
            } else {
                gravatar_alternative
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitMembership<U, K> {
    pub id: i32,
    pub user: U,
    pub kit: K,
    pub datetime_linked: DateTime<Utc>,
    pub access_super: bool,
    pub access_configure: bool,
}

impl<U, K> KitMembership<U, K> {
    pub fn with_kit<NK>(self, kit: NK) -> KitMembership<U, NK> {
        KitMembership {
            id: self.id,
            user: self.user,
            kit,
            datetime_linked: self.datetime_linked,
            access_super: self.access_super,
            access_configure: self.access_configure,
        }
    }

    pub fn with_user<NU>(self, user: NU) -> KitMembership<NU, K> {
        KitMembership {
            id: self.id,
            user,
            kit: self.kit,
            datetime_linked: self.datetime_linked,
            access_super: self.access_super,
            access_configure: self.access_configure,
        }
    }
}

impl From<models::KitMembership> for KitMembership<i32, i32> {
    fn from(
        models::KitMembership {
            id,
            user_id,
            kit_id,
            datetime_linked,
            access_super,
            access_configure,
        }: models::KitMembership,
    ) -> Self {
        Self {
            id,
            user: user_id,
            kit: kit_id,
            datetime_linked,
            access_super,
            access_configure,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralDefinition {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub module_name: String,
    pub class_name: String,
    pub configuration_schema: serde_json::Value,
}

impl From<models::PeripheralDefinition> for PeripheralDefinition {
    fn from(peripheral_definition: models::PeripheralDefinition) -> Self {
        let models::PeripheralDefinition {
            id,
            name,
            description,
            brand,
            model,
            module_name,
            class_name,
            configuration_schema,
        } = peripheral_definition;
        Self {
            id,
            name,
            description,
            brand,
            model,
            module_name,
            class_name,
            configuration_schema,
        }
    }
}
