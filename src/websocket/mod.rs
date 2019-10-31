use log::info;

use super::{helpers, models, views, PgPool, PgPooled};

use astroplant_mqtt::{MqttApiMessage, ServerRpcRequest};
use futures::channel::{mpsc, oneshot};
use futures::future::FutureExt;
use futures::stream::StreamExt;
use serde::Serialize;
use tokio::runtime::{Runtime, TaskExecutor};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawMeasurement {
    pub kit_serial: String,
    pub datetime: u64,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub value: f64,
}

pub async fn run(mut raw_measurement_receiver: mpsc::Receiver<astroplant_mqtt::RawMeasurement>) {
    info!("Starting WebSocket server.");
    let mut publisher = astroplant_websocket::run();

    while let Some(raw_measurement) = raw_measurement_receiver.next().await {
        let astroplant_mqtt::RawMeasurement {
            kit_serial,
            datetime,
            peripheral,
            quantity_type,
            value,
            ..
        } = raw_measurement;
        let raw_measurement = RawMeasurement {
            kit_serial,
            datetime,
            peripheral,
            quantity_type,
            value,
        };

        publisher.publish_raw_measurement(
            raw_measurement.kit_serial.clone(),
            serde_json::to_value(raw_measurement).unwrap(),
        )
    }
}
