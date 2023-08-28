use diesel::prelude::*;
use diesel::PgConnection;

use crate::{
    models::{NewPeripheralDefinition, QuantityTypeId},
    schema::{
        peripheral_definition_expected_quantity_types, peripheral_definitions, quantity_types,
    },
};

use crate::models::{PeripheralDefinition, QuantityType};

pub enum InsertSimulationDefinitions {
    Yes,
    No,
}

impl From<bool> for InsertSimulationDefinitions {
    fn from(value: bool) -> Self {
        if value {
            InsertSimulationDefinitions::Yes
        } else {
            InsertSimulationDefinitions::No
        }
    }
}

#[derive(Clone, Copy)]
pub enum ExistingPeripheralDefinitionStrategy {
    Skip,
    Update,
}

#[derive(Clone, Copy)]
struct QuantityTypes {
    temperature: QuantityTypeId,
    pressure: QuantityTypeId,
    humidity: QuantityTypeId,
    concentration: QuantityTypeId,
    light_intensity: QuantityTypeId,
}

fn select_or_insert_qt(
    conn: &mut PgConnection,
    physical_quantity: &str,
    physical_unit: &str,
    physical_unit_symbol: Option<&str>,
) -> anyhow::Result<QuantityTypeId> {
    use crate::schema::quantity_types::dsl;
    use diesel::upsert::*;

    let qt_ = (
        dsl::physical_quantity.eq(physical_quantity),
        dsl::physical_unit.eq(physical_unit),
        dsl::physical_unit_symbol.eq(physical_unit_symbol),
    );
    // Insert the quantity type, updating the quantity type if the (quantity,unit) pair already
    // exists.
    let qt: QuantityType = diesel::dsl::insert_into(quantity_types::table)
        .values(&qt_)
        .on_conflict(on_constraint(
            "quantity_types_physical_quantity_physical_unit_key",
        ))
        .do_update()
        .set(qt_)
        .get_result(conn)?;

    Ok(qt.get_id())
}

fn insert_pd(
    conn: &mut PgConnection,
    peripheral_definition: NewPeripheralDefinition,
    expected_quantity_types: &[QuantityTypeId],
    existing_peripheral_definition_strategy: ExistingPeripheralDefinitionStrategy,
) -> anyhow::Result<()> {
    let pd: PeripheralDefinition = match existing_peripheral_definition_strategy {
        ExistingPeripheralDefinitionStrategy::Skip => {
            match diesel::dsl::insert_into(peripheral_definitions::table)
                .values(&peripheral_definition)
                .on_conflict_do_nothing()
                .get_result(conn)
                .optional()?
            {
                Some(pd) => pd,
                None => {
                    // definition already exists
                    tracing::info!(
                        "Peripheral definition '{name}' was not inserted: it already exists",
                        name = &peripheral_definition.name,
                    );
                    return Ok(());
                }
            }
        }
        ExistingPeripheralDefinitionStrategy::Update => {
            diesel::dsl::insert_into(peripheral_definitions::table)
                .values(&peripheral_definition)
                .on_conflict(peripheral_definitions::name)
                .do_update()
                .set(&peripheral_definition)
                .get_result(conn)?
        }
    };
    let pid = pd.get_id();

    // this bumps ids of the expected quantity types
    // (but perhaps ids should be removed from that table anyway, simply using a tuple of (pid, qtid))
    diesel::dsl::delete(peripheral_definition_expected_quantity_types::table).filter(
        crate::schema::peripheral_definition_expected_quantity_types::dsl::peripheral_definition_id
            .eq(pid.0),
    ).execute(conn)?;
    diesel::dsl::insert_into(peripheral_definition_expected_quantity_types::table)
        .values(
            expected_quantity_types
                .iter()
                .map(|qid| {
                    use crate::schema::peripheral_definition_expected_quantity_types::dsl;

                    (
                        dsl::quantity_type_id.eq(qid.0),
                        dsl::peripheral_definition_id.eq(pid.0),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .execute(conn)?;

    Ok(())
}

pub fn insert_astroplant_definitions(
    conn: &mut PgConnection,
    simulation_definitions: InsertSimulationDefinitions,
    existing_peripheral_definition_strategy: ExistingPeripheralDefinitionStrategy,
) -> anyhow::Result<()> {
    tracing::info!("Inserting astroplant definitions");

    let config_measurement_interval = serde_json::json!({
        "type": "object",
        "title": "Intervals",
        "required": ["measurementInterval", "aggregateInterval"],
        "properties": {
            "measurementInterval": {
                "title": "Measurement interval",
                "description": "The interval in seconds between measurements.",
                "type": "integer",
                "minimum": 5,
                "default": 60,
            },
            "aggregateInterval": {
                "title": "Aggregate interval",
                "description": "The interval in seconds between measurement aggregation.",
                "type": "integer",
                "minimum": 60,
                "default": 60 * 30,
            },
        },
    });

    conn.transaction(|conn| {
        let temperature = select_or_insert_qt(conn, "Temperature", "Degrees Celsius", Some("°C"))?;
        let pressure = select_or_insert_qt(conn, "Pressure", "Hectopascal", Some("hPa"))?;
        let humidity = select_or_insert_qt(conn, "Humidity", "Percent", Some("%"))?;
        let concentration =
            select_or_insert_qt(conn, "Concentration", "Parts per million", Some("PPM"))?;
        let light_intensity = select_or_insert_qt(conn, "Light intensity", "Lux", Some("lx"))?;

        let quantity_types = QuantityTypes {
            temperature,
            pressure,
            humidity,
            concentration,
            light_intensity,
        };

        if matches!(simulation_definitions, InsertSimulationDefinitions::Yes) {
            insert_simulation_definitions(
                conn,
                quantity_types,
                &config_measurement_interval,
                existing_peripheral_definition_strategy,
            )?;
        }

        insert_astroplant_definitions_(
            conn,
            quantity_types,
            &config_measurement_interval,
            existing_peripheral_definition_strategy,
        )?;

        anyhow::Ok(())
    })?;

    Ok(())
}

fn insert_simulation_definitions(
    conn: &mut PgConnection,
    quantity_types: QuantityTypes,
    config_measurement_interval: &serde_json::Value,
    existing_peripheral_definition_strategy: ExistingPeripheralDefinitionStrategy,
) -> anyhow::Result<()> {
    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Virtual temperature sensor".to_owned(),
            description: Some(
                "A virtual temperature sensor using the environment simulation.".to_owned(),
            ),
            brand: Some("AstroPlant Virtual".to_owned()),
            model: Some("Temperature".to_owned()),
            symbol_location: "astroplant_simulation.sensors".to_owned(),
            symbol: "Temperature".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals"],
                "properties": {"intervals": config_measurement_interval},
            }),
            command_schema: None,
        },
        &[quantity_types.temperature],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Virtual pressure sensor".to_owned(),
            description: Some(
                "A virtual pressure sensor using the environment simulation.".to_owned(),
            ),
            brand: Some("AstroPlant Virtual".to_owned()),
            model: Some("Pressure".to_owned()),
            symbol_location: "astroplant_simulation.sensors".to_owned(),
            symbol: "Pressure".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals"],
                "properties": {"intervals": config_measurement_interval},
            }),
            command_schema: None,
        },
        &[quantity_types.pressure],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Virtual barometer".to_owned(),
            description: Some("A virtual barometer using the environment simulation.".to_owned()),
            brand: Some("AstroPlant Virtual".to_owned()),
            model: Some("Barometer".to_owned()),
            symbol_location: "astroplant_simulation.sensors".to_owned(),
            symbol: "Barometer".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals"],
                "properties": {"intervals": config_measurement_interval},
            }),
            command_schema: None,
        },
        &[
            quantity_types.temperature,
            quantity_types.pressure,
            quantity_types.humidity,
        ],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Virtual heater".to_owned(),
            description: Some("A virtual heater using the environment simulation.".to_owned()),
            brand: Some("AstroPlant Virtual".to_owned()),
            model: Some("Heater".to_owned()),
            symbol_location: "astroplant_simulation.actuators".to_owned(),
            symbol: "Heater".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "null",
                "default": null,
            }),
            command_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "heat": {
                        "type": "number",
                        "title": "Heater intensity",
                        "minimum": 0,
                        "maximum": 10,
                    },
                },
            })),
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Virtual random camera".to_owned(),
            description: Some(
                "A virtual camera to generate random images: uniform noise or abstract art."
                    .to_owned(),
            ),
            brand: Some("AstroPlant Virtual".to_owned()),
            model: Some("Random Camera".to_owned()),
            symbol_location: "astroplant_simulation.cameras".to_owned(),
            symbol: "Random".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["schedule"],
                "properties": {
                    "schedule": {
                        "type": "array",
                        "title": "Capture schedule",
                        "items": {
                            "type": "object",
                            "required": ["time", "command"],
                            "properties": {
                                "time": {
                                    "type": "string",
                                    "format": "time",
                                    "title": "Time",
                                },
                                "command": {
                                    "type": "string",
                                    "enum": ["uniform", "art"],
                                    "title": "Command",
                                },
                            },
                        },
                    },
                },
            }),
            command_schema: Some(serde_json::json!({
                "type": "string",
                "enum": ["art", "uniform"],
            })),
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    Ok(())
}

fn insert_astroplant_definitions_(
    conn: &mut PgConnection,
    quantity_types: QuantityTypes,
    config_measurement_interval: &serde_json::Value,
    existing_peripheral_definition_strategy: ExistingPeripheralDefinitionStrategy,
) -> anyhow::Result<()> {
    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "Local data logger".to_owned(),
            description: Some("Logs aggregate measurement data locally.".to_owned()),
            brand: Some("AstroPlant".to_owned()),
            model: Some("Logger".to_owned()),
            symbol_location: "peripheral".to_owned(),
            symbol: "LocalDataLogger".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["storagePath"],
                "properties": {
                    "storagePath": {
                        "title": "Storage path",
                        "description": "The path to store log files locally. Either absolute, or relative to the program working directory.",
                        "type": "string",
                        "default": "./data",
                    }
                },
            }),
            command_schema: None,
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant air temperature sensor".to_owned(),
            description: Some("Measures air temperature and humidity.".to_owned()),
            brand: Some("Asair".to_owned()),
            model: Some("AM2302".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.dht22".to_owned(),
            symbol: "Dht22".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals", "gpioAddress"],
                "properties": {
                    "intervals": config_measurement_interval,
                    "gpioAddress": {
                        "title": "GPIO address",
                        "description": "The pin number the sensor is connected to.",
                        "type": "integer",
                        "default": 17,
                    },
                },
            }),
            command_schema: None,
        },
        &[quantity_types.temperature, quantity_types.humidity],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant air CO² sensor".to_owned(),
            description: Some("Measures carbon dioxide concentration in the air.".to_owned()),
            brand: None,
            model: Some("MH-Z19".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.mh_z19".to_owned(),
            symbol: "MhZ19".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals", "serialDeviceFile"],
                "properties": {
                    "intervals": config_measurement_interval,
                    "serialDeviceFile": {
                        "title": "Serial device",
                        "description": "The device file name of the serial interface.",
                        "type": "string",
                        "default": "/dev/ttyS0",
                    },
                },
            }),
            command_schema: None,
        },
        &[quantity_types.concentration],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant light sensor".to_owned(),
            description: Some("Measures light intensity.".to_owned()),
            brand: None,
            model: Some("BH1750".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.bh1750".to_owned(),
            symbol: "Bh1750".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals", "i2cAddress"],
                "properties": {
                    "intervals": config_measurement_interval,
                    "i2cAddress": {
                        "title": "I2C address",
                        "description": "The I2C address of this device.",
                        "type": "string",
                        "default": "0x23",
                    },
                },
            }),
            command_schema: None,
        },
        &[quantity_types.light_intensity],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant water temperature sensor".to_owned(),
            description: Some("Measures water temperature.".to_owned()),
            brand: None,
            model: Some("DS18B20".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.ds18b20".to_owned(),
            symbol: "Ds18b20".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["intervals"],
                "properties": {
                    "intervals": config_measurement_interval,
                    "oneWireDeviceId": {
                        "type": "string",
                        "title": "1-Wire device identifier",
                        "description": "The 1-Wire device ID to filter on. Keep blank to not filter on device IDs.",
                    },
                },
            }),
            command_schema: None,
        },
        &[quantity_types.temperature],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant fans".to_owned(),
            description: Some("AstroPlant fans controlled through PWM.".to_owned()),
            brand: None,
            model: Some("24V DC".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.pwm".to_owned(),
            symbol: "Pwm".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["gpioAddresses"],
                "properties": {
                    "gpioAddresses": {
                        "title": "GPIO addresses",
                        "description": "The GPIO addresses this logical fan device controls.",
                        "type": "array",
                        "items": {"type": "integer"},
                        "default": [16, 19],
                    }
                },
            }),
            command_schema: Some(serde_json::json!({
                "type": "object",
                "required": [],
                "properties": {
                    "intensity": {
                        "type": "number",
                        "title": "Fan intensity",
                        "description": "The fan intensity as a percentage of full power.",
                        "minimum": 0,
                        "maximum": 100,
                    },
                },
            })),
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant LED panel".to_owned(),
            description: Some("AstroPlant LED panel controlled through PWM.".to_owned()),
            brand: Some("AstroPlant".to_owned()),
            model: Some("mk06".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.led_panel".to_owned(),
            symbol: "LedPanel".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["gpioAddressBlue", "gpioAddressRed", "gpioAddressFarRed"],
                "properties": {
                    "gpioAddressBlue": {
                        "title": "GPIO address blue",
                        "description": "The GPIO address for PWM control of the blue LED.",
                        "type": "integer",
                        "default": 21,
                    },
                    "gpioAddressRed": {
                        "title": "GPIO address red",
                        "description": "The GPIO address for PWM control of the red LED.",
                        "type": "integer",
                        "default": 20,
                    },
                    "gpioAddressFarRed": {
                        "title": "GPIO address far-red",
                        "description": "The GPIO address for PWM control of the far-red LED.",
                        "type": "integer",
                        "default": 18,
                    },
                },
            }),
            command_schema: Some(serde_json::json!({
                "type": "object",
                "title": "LED brightness",
                "description": "The LED brightnesses as percentage of full brightness.",
                "required": [],
                "properties": {
                    "blue": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                    },
                    "red": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                    },
                    "farRed": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                    },
                },
            })),
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    insert_pd(
        conn,
        NewPeripheralDefinition {
            name: "AstroPlant Camera".to_owned(),
            description: Some("AstroPlant camera control implementation for the Raspberry Pi V2 cameras (Normal and NoIR) for normal, NIR and NDVI captures.".to_owned()),
            brand: Some("Raspberry".to_owned()),
            model: Some("Camera V2".to_owned()),
            symbol_location: "astroplant_peripheral_device_library.pi_camera_v2".to_owned(),
            symbol: "PiCameraV2".to_owned(),
            configuration_schema: serde_json::json!({
                "type": "object",
                "title": "Configuration",
                "required": ["camera", "schedule"],
                "properties": {
                    "camera": {
                        "type": "string",
                        "enum": ["piCameraV2"],
                    },
                    "schedule": {
                        "type": "array",
                        "title": "Capture schedule",
                        "items": {
                            "type": "object",
                            "required": ["time", "command"],
                            "properties": {
                                "time": {
                                    "type": "string",
                                    "format": "time",
                                    "title": "Time",
                                },
                                "command": {
                                    "type": "string",
                                    "enum": ["uncontrolled", "regular", "nir", "ndvi"],
                                    "title": "Command",
                                },
                            },
                        },
                    },
                },
            }),
            command_schema: Some(serde_json::json!({
                "type": "string",
                "enum": ["uncontrolled", "regular", "nir", "ndvi"],
                "title": "Command",
            })),
        },
        &[],
        existing_peripheral_definition_strategy,
    )?;

    Ok(())
}
