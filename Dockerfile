FROM rust:1.51 as builder

RUN apt-get update && apt-get install capnproto -y
WORKDIR /usr/src/astroplant-api

COPY Cargo.lock .
COPY Cargo.toml .
COPY astroplant-auth ./astroplant-auth
COPY astroplant-mqtt ./astroplant-mqtt
COPY astroplant-object ./astroplant-object
COPY astroplant-websocket ./astroplant-websocket
COPY random-string ./random-string
COPY src ./src
RUN cargo build --release

FROM debian:buster-slim

RUN apt-get update && apt-get install libpq5 -y
COPY --from=builder /usr/src/astroplant-api/target/release/astroplant-api /usr/local/bin/astroplant-api
RUN head -n 256 /dev/urandom > /token_signer.key

ENV DATABASE_URL=
ENV MQTT_HOST=mqtt.ops
ENV MQTT_PORT=1883
ENV MQTT_USERNAME=
ENV MQTT_PASSWORD=
ENV AWS_S3_REGION=
ENV AWS_S3_ENDPOINT=
ENV AWS_ACCESS_KEY_ID=
ENV AWS_SECRET_ACCESS_KEY=
ENV AWS_SESSION_TOKEN=
ENV AWS_CREDENTIAL_EXPIRATION=
ENV RUST_BACKTRACE=1
ENV RUST_LOG=warn,astroplant_api=debug
ENV TOKEN_SIGNER_KEY=/token_signer.key

EXPOSE 8080

CMD ["astroplant-api"]
