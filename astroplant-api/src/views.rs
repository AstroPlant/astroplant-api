use crate::models;
use crate::problem::{Problem, INTERNAL_SERVER_ERROR};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;

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
    pub last_seen: Option<DateTime<Utc>>,
}

impl From<(models::Kit, Option<DateTime<Utc>>)> for Kit {
    fn from((kit, last_seen): (models::Kit, Option<DateTime<Utc>>)) -> Self {
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
            last_seen,
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
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
    pub symbol_location: String,
    pub symbol: String,
    pub configuration_schema: serde_json::Value,
    pub command_schema: Option<serde_json::Value>,
}

impl PeripheralDefinition {
    pub fn with_expected_quantity_types<Q>(
        self,
        expected_quantity_types: Vec<Q>,
    ) -> PeripheralDefinitionWithExpectedQuantityTypes<Q> {
        PeripheralDefinitionWithExpectedQuantityTypes {
            peripheral_definition: self,
            expected_quantity_types,
        }
    }
}

impl From<models::PeripheralDefinition> for PeripheralDefinition {
    fn from(peripheral_definition: models::PeripheralDefinition) -> Self {
        let models::PeripheralDefinition {
            id,
            name,
            description,
            brand,
            model,
            symbol_location,
            symbol,
            configuration_schema,
            command_schema,
        } = peripheral_definition;
        Self {
            id,
            name,
            description,
            brand,
            model,
            symbol_location,
            symbol,
            configuration_schema,
            command_schema,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralDefinitionWithExpectedQuantityTypes<Q> {
    #[serde(flatten)]
    pub peripheral_definition: PeripheralDefinition,
    pub expected_quantity_types: Vec<Q>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralDefinitionExpectedQuantityType<P, Q> {
    pub id: i32,
    pub peripheral_definition: P,
    pub quantity_type: Q,
}

impl From<models::PeripheralDefinitionExpectedQuantityType>
    for PeripheralDefinitionExpectedQuantityType<i32, i32>
{
    fn from(expected_quantity_type: models::PeripheralDefinitionExpectedQuantityType) -> Self {
        let models::PeripheralDefinitionExpectedQuantityType {
            id,
            peripheral_definition_id,
            quantity_type_id,
        } = expected_quantity_type;
        Self {
            id,
            peripheral_definition: peripheral_definition_id,
            quantity_type: quantity_type_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuantityType {
    pub id: i32,
    pub physical_quantity: String,
    pub physical_unit: String,
    pub physical_unit_symbol: Option<String>,
}

impl From<models::QuantityType> for QuantityType {
    fn from(quantity_type: models::QuantityType) -> Self {
        let models::QuantityType {
            id,
            physical_quantity,
            physical_unit,
            physical_unit_symbol,
        } = quantity_type;
        Self {
            id,
            physical_quantity,
            physical_unit,
            physical_unit_symbol,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitConfiguration {
    pub id: i32,
    pub kit_id: i32,
    pub description: Option<String>,
    pub controller_symbol_location: String,
    pub controller_symbol: String,
    pub control_rules: serde_json::Value,
    pub active: bool,
    pub never_used: bool,
}

impl KitConfiguration {
    pub fn with_peripherals<P>(self, peripherals: Vec<P>) -> KitConfigurationWithPeripherals<P> {
        KitConfigurationWithPeripherals {
            kit_configuration: self,
            peripherals,
        }
    }
}

impl From<models::KitConfiguration> for KitConfiguration {
    fn from(
        models::KitConfiguration {
            id,
            kit_id,
            description,
            controller_symbol_location,
            controller_symbol,
            control_rules,
            active,
            never_used,
            ..
        }: models::KitConfiguration,
    ) -> Self {
        Self {
            id,
            kit_id,
            description,
            controller_symbol_location,
            controller_symbol,
            control_rules,
            active,
            never_used,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitConfigurationWithPeripherals<P> {
    #[serde(flatten)]
    pub kit_configuration: KitConfiguration,
    pub peripherals: Vec<P>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Peripheral {
    pub id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub peripheral_definition_id: i32,
    pub name: String,
    pub configuration: serde_json::Value,
}

impl Peripheral {
    pub fn with_definition(self, definition: PeripheralDefinition) -> PeripheralWithDefinition {
        PeripheralWithDefinition {
            peripheral: self,
            definition,
        }
    }
}

impl From<models::Peripheral> for Peripheral {
    fn from(
        models::Peripheral {
            id,
            kit_id,
            kit_configuration_id,
            peripheral_definition_id,
            name,
            configuration,
            ..
        }: models::Peripheral,
    ) -> Self {
        Self {
            id,
            kit_id,
            kit_configuration_id,
            peripheral_definition_id,
            name,
            configuration,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralWithDefinition {
    pub peripheral: Peripheral,
    pub definition: PeripheralDefinition,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMeasurement {
    pub id: uuid::Uuid,
    pub peripheral_id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub quantity_type_id: i32,
    pub datetime_start: DateTime<Utc>,
    pub datetime_end: DateTime<Utc>,
    pub values: HashMap<String, f64>,
}

impl TryFrom<models::AggregateMeasurement> for AggregateMeasurement {
    type Error = Problem;

    fn try_from(
        models::AggregateMeasurement {
            id,
            peripheral_id,
            kit_id,
            kit_configuration_id,
            quantity_type_id,
            datetime_start,
            datetime_end,
            values,
            ..
        }: models::AggregateMeasurement,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id,
            peripheral_id,
            kit_id,
            kit_configuration_id,
            quantity_type_id,
            datetime_start,
            datetime_end,
            values: serde_json::from_value(values).map_err(|_| INTERNAL_SERVER_ERROR)?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: uuid::Uuid,
    pub peripheral_id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub datetime: DateTime<Utc>,
    pub name: String,
    pub r#type: String,
    pub metadata: serde_json::Value,
    pub size: i64,
}

impl TryFrom<models::Media> for Media {
    type Error = Problem;

    fn try_from(
        models::Media {
            id,
            peripheral_id,
            kit_id,
            kit_configuration_id,
            datetime,
            name,
            r#type,
            metadata,
            size,
            ..
        }: models::Media,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id,
            peripheral_id,
            kit_id,
            kit_configuration_id,
            datetime,
            name,
            r#type,
            metadata,
            size,
        })
    }
}
