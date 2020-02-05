use log::info;

use super::{helpers, models, views, PgPool, PgPooled};

use astroplant_mqtt::{MqttApiMessage, ServerRpcRequest};
use futures::channel::{mpsc, oneshot};
use futures::future::FutureExt;
use futures::stream::StreamExt;
use serde::Serialize;
use tokio::runtime::{Runtime, TaskExecutor};

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
        let raw_measurement = astroplant_websocket::RawMeasurement {
            kit_serial,
            datetime,
            peripheral,
            quantity_type,
            value,
        };

        publisher.publish_raw_measurement(raw_measurement.kit_serial.clone(), raw_measurement)
    }
}
