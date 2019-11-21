use astroplant_mqtt::{KitRpc, ServerRpcRequest};

fn main() {
    let (receiver, kits_rpc) = astroplant_mqtt::run(
        "mqtt.ops".to_owned(),
        1883,
        "server".to_owned(),
        "abcdef".to_owned(),
    );

    std::thread::spawn(move || {
        println!("Querying kit with serial 'k_develop'");
        let kit_rpc: KitRpc = kits_rpc.kit_rpc("k_develop".to_owned());
        println!(
            "Version response: {:?}",
            futures::executor::block_on(kit_rpc.version())
        );
        println!(
            "Uptime response: {:?}",
            futures::executor::block_on(kit_rpc.uptime())
        );
    });

    while let Ok(res) = receiver.recv() {
        println!("Received request: {:?}", res);
        if let astroplant_mqtt::MqttApiMessage::ServerRpcRequest(rpc_request) = res {
            match rpc_request {
                ServerRpcRequest::Version { response } => {
                    response
                        .send("astroplant-mqtt-bin-tester".to_owned())
                        .unwrap();
                }
                _ => {}
            }
        }
    }

    println!("Disconnected")
}
