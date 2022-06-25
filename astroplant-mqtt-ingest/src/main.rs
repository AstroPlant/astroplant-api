use futures::StreamExt;
use std::rc::Rc;

use astroplant_mqtt::{AggregateMeasurement, Message, RawMeasurement};

mod database;
mod task;
use task::LocalTaskPool;

static DEFAULT_DATABASE_URL: &str =
    "postgres://astroplant:astroplant@localhost/astroplant?connect_timeout=5";
static DEFAULT_MQTT_HOST: &str = "localhost";
const DEFAULT_MQTT_PORT: u16 = 1883;

async fn ingest_raw_measurement(
    db: Rc<database::Db>,
    raw_measurement: RawMeasurement,
) -> anyhow::Result<()> {
    db.insert_raw(raw_measurement).await?;
    Ok(())
}

async fn ingest_aggregate_measurement(
    db: Rc<database::Db>,
    aggregate_measurement: AggregateMeasurement,
) -> anyhow::Result<()> {
    db.insert_aggregate(aggregate_measurement).await?;
    Ok(())
}

/// # Panics
///
/// This panics if it is run outside of a [LocalSet](tokio::task::LocalSet) context.
async fn ingest<H>(
    mqtt_connection: astroplant_mqtt::Connection<H>,
    db: database::Db,
) -> anyhow::Result<()>
where
    H: astroplant_mqtt::ServerRpcHandler + Send + Sync + 'static,
{
    let mut mqtt_stream = mqtt_connection.into_stream();
    let db = Rc::new(db);

    let task_queue = LocalTaskPool::start(8);
    while let Some(message) = mqtt_stream.next().await {
        match message {
            Ok(Message::RawMeasurement(raw_measurement)) => {
                task_queue
                    .enqueue(ingest_raw_measurement(db.clone(), raw_measurement))
                    .await;
            }
            Ok(Message::AggregateMeasurement(aggregate_measurement)) => {
                task_queue
                    .enqueue(ingest_aggregate_measurement(
                        db.clone(),
                        aggregate_measurement,
                    ))
                    .await;
            }
            Err(astroplant_mqtt::Error::Mqtt(e)) => {
                anyhow::bail!(e);
            }
            Err(astroplant_mqtt::Error::MqttClientError(e)) => {
                anyhow::bail!(e);
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut builder = astroplant_mqtt::ConnectionBuilder::new(
        std::env::var("MQTT_HOST").unwrap_or(crate::DEFAULT_MQTT_HOST.to_owned()),
        std::env::var("MQTT_PORT")
            .map_err(|_| ())
            .and_then(|port| port.parse().map_err(|_| ()))
            .unwrap_or(crate::DEFAULT_MQTT_PORT),
    );
    builder = builder.with_client_id("astroplant-mqtt-ingest");

    if let Ok(username) = std::env::var("MQTT_USERNAME") {
        builder = builder.with_credentials(
            username,
            std::env::var("MQTT_PASSWORD").unwrap_or("".to_string()),
        );
    }

    let (mqtt_connection, _) = builder.create();

    let (db_client, db_connection) = tokio_postgres::connect(
        std::env::var("DATABASE_URL")
            .as_deref()
            .unwrap_or(DEFAULT_DATABASE_URL),
        tokio_postgres::NoTls,
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = db_connection.await {
            // TODO: exit on DB connection error;
            tracing::error!("A database connection error occurred: {:?}", e);
        }
    });

    let db = database::Db::new(db_client).await?;

    let local = tokio::task::LocalSet::new();

    tracing::info!("MQTT ingest started");

    local.run_until(ingest(mqtt_connection, db)).await?;
    tracing::info!("MQTT ingest shutting down");

    // Poll all remaining tasks to completion.
    local.await;

    tracing::info!("MQTT ingest stopped");
    Ok(())
}
