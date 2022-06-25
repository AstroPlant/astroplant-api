use std::convert::TryFrom;
use std::{cell::RefCell, collections::HashMap};
use tokio_postgres::types::Type;
use tokio_postgres::{Client, Statement};

use astroplant_mqtt::{AggregateMeasurement, RawMeasurement};

type KitSerial = String;
type PeripheralId = i32;
type KitId = i32;
type ConfigId = i32;

#[derive(Debug)]
struct KitAndConfigId {
    kit_serial: String,
    kit_id: KitId,
    config_id: ConfigId,
}

#[derive(Debug)]
struct KitIdAndConfigId {
    kit_id: KitId,
    config_id: ConfigId,
}

pub(crate) struct Db {
    config_cache: RefCell<HashMap<PeripheralId, KitAndConfigId>>,
    client: Client,
    get_config_and_kit: Statement,
    insert_raw_measurement: Statement,
    insert_aggregate_measurement: Statement,
}

const GET_CONFIG_AND_KIT: &str = "
    SELECT peripherals.kit_configuration_id AS config_id, kits.id AS kit_id, kits.serial AS kit_serial
    FROM peripherals
    JOIN kits ON (peripherals.kit_id = kits.id)
    WHERE peripherals.id = $1
";

const INSERT_RAW_MEASUREMENT: &str = "
    INSERT INTO raw_measurements (id, peripheral_id, kit_id, kit_configuration_id, quantity_type_id, value, datetime)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
";

const INSERT_AGGREGATE_MEASUREMENT: &str = "
    INSERT INTO aggregate_measurements (id, peripheral_id, kit_id, kit_configuration_id, quantity_type_id, datetime_start, datetime_end, values)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
";

impl Db {
    pub(crate) async fn new(client: Client) -> anyhow::Result<Self> {
        let get_config_and_kit = client
            .prepare_typed(GET_CONFIG_AND_KIT, &[Type::INT4])
            .await?;

        let insert_raw_measurement = client
            .prepare_typed(
                INSERT_RAW_MEASUREMENT,
                &[
                    Type::UUID,
                    Type::INT4,
                    Type::INT4,
                    Type::INT4,
                    Type::INT4,
                    Type::FLOAT8,
                    Type::TIMESTAMPTZ,
                ],
            )
            .await?;

        let insert_aggregate_measurement = client
            .prepare_typed(
                INSERT_AGGREGATE_MEASUREMENT,
                &[
                    Type::UUID,
                    Type::INT4,
                    Type::INT4,
                    Type::INT4,
                    Type::INT4,
                    Type::TIMESTAMPTZ,
                    Type::TIMESTAMPTZ,
                    Type::JSON,
                ],
            )
            .await?;

        let db = Self {
            config_cache: RefCell::new(HashMap::new()),
            client,
            get_config_and_kit,
            insert_raw_measurement,
            insert_aggregate_measurement,
        };
        Ok(db)
    }

    async fn config_and_kit(
        &self,
        peripheral_id: PeripheralId,
        kit_serial: &str,
    ) -> anyhow::Result<Option<KitIdAndConfigId>> {
        let in_cache = self.config_cache.borrow().contains_key(&peripheral_id);

        if !in_cache {
            let res = match self
                .client
                .query_opt(&self.get_config_and_kit, &[&peripheral_id])
                .await?
            {
                Some(res) => res,
                None => return Ok(None),
            };

            let config_id: i32 = res.get("config_id");
            let kit_id: i32 = res.get("kit_id");
            let kit_serial_: String = res.get("kit_serial");

            let mut config_cache = self.config_cache.borrow_mut();
            config_cache.insert(
                peripheral_id,
                KitAndConfigId {
                    kit_serial: kit_serial_,
                    kit_id,
                    config_id,
                },
            );
        }

        let config = self
            .config_cache
            .borrow()
            .get(&peripheral_id)
            .and_then(|kit_and_config_id| {
                if kit_and_config_id.kit_serial == kit_serial {
                    Some(KitIdAndConfigId {
                        kit_id: kit_and_config_id.kit_id,
                        config_id: kit_and_config_id.config_id,
                    })
                } else {
                    None
                }
            });
        Ok(config)
    }

    pub(crate) async fn insert_raw(&self, raw: RawMeasurement) -> anyhow::Result<()> {
        let config = match self.config_and_kit(raw.peripheral, &raw.kit_serial).await? {
            Some(config) => config,
            None => {
                tracing::trace!(
                    "Ignoring raw measurement {} of kit {} and peripheral {}: the stated peripheral does not belong to the kit",
                    raw.id,
                    raw.kit_serial,
                    raw.peripheral,
                );
                return Ok(());
            }
        };

        self.client
            .query(
                &self.insert_raw_measurement,
                &[
                    &raw.id,
                    &raw.peripheral,
                    &config.kit_id,
                    &config.config_id,
                    &raw.quantity_type,
                    &raw.value,
                    &raw.datetime,
                ],
            )
            .await?;

        tracing::trace!(
            "Inserted raw measurement {} of kit {} and peripheral {}",
            raw.id,
            raw.kit_serial,
            raw.peripheral,
        );

        Ok(())
    }

    pub(crate) async fn insert_aggregate(&self, raw: AggregateMeasurement) -> anyhow::Result<()> {
        let config = match self.config_and_kit(raw.peripheral, &raw.kit_serial).await? {
            Some(config) => config,
            None => {
                tracing::trace!(
                    "Ignoring aggregate measurement {} of kit {} and peripheral {}: the stated peripheral does not belong to the kit",
                    raw.id,
                    raw.kit_serial,
                    raw.peripheral,
                );
                return Ok(());
            }
        };

        self.client
            .query(
                &self.insert_aggregate_measurement,
                &[
                    &raw.id,
                    &raw.peripheral,
                    &config.kit_id,
                    &config.config_id,
                    &raw.quantity_type,
                    &raw.datetime_start,
                    &raw.datetime_end,
                    &serde_json::to_value(raw.values)?,
                ],
            )
            .await?;

        tracing::trace!(
            "Inserted aggregate measurement {} of kit {} and peripheral {}",
            raw.id,
            raw.kit_serial,
            raw.peripheral,
        );

        Ok(())
    }
}
