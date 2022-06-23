use futures::StreamExt;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (connection, kits_rpc) = astroplant_mqtt::ConnectionBuilder::new("localhost", 1883)
        .with_credentials("server", "abcdef")
        .create();

    let mut stream = connection.into_stream();

    while let Some(msg) = stream.next().await {
        println!("{:?}", msg);
        match msg {
            Ok(astroplant_mqtt::Message::RawMeasurement(measurement)) => {
                let rpc = kits_rpc.clone();
                tokio::spawn(async move {
                    println!("Version: {:?}", rpc.version(&measurement.kit_serial).await);
                    println!("Uptime:  {:?}", rpc.uptime(&measurement.kit_serial).await);
                });
            }
            _ => {}
        }
    }
}
