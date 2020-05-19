FROM rust:1.43

RUN apt-get update && apt-get install capnproto -y
WORKDIR /usr/src/astroplant-api
COPY . .
RUN cargo build --release

ENV DATABASE_URL=
ENV MQTT_HOST=mqtt.ops
ENV MQTT_PORT=1883
ENV MQTT_USERNAME=
ENV MQTT_PASSWORD=
ENV RUST_BACKTRACE=1
ENV RUST_LOG=warn,astroplant_rs_api=debug

EXPOSE 8080

CMD ["./target/release/astroplant-api"]
