use log::info;

use futures::channel::mpsc;
use futures::stream::StreamExt;

pub async fn run(
    mut publisher: astroplant_websocket::WebSocketPublisher,
    mut raw_measurement_receiver: mpsc::Receiver<astroplant_mqtt::RawMeasurement>,
) {
    info!("Starting WebSocket server.");

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
