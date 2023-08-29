FROM rust:1.70 as builder

RUN apt-get update && apt-get install capnproto -y
WORKDIR /usr/src/astroplant-api

COPY Cargo.lock .
COPY Cargo.toml .
COPY sqlx-data.json .
COPY astroplant-api ./astroplant-api
COPY astroplant-auth ./astroplant-auth
COPY astroplant-mqtt ./astroplant-mqtt
COPY astroplant-mqtt-ingest ./astroplant-mqtt-ingest
COPY astroplant-object ./astroplant-object
COPY astroplant-websocket ./astroplant-websocket
COPY migrations ./migrations
COPY random-string ./random-string

RUN cargo build --release --package astroplant-api
RUN cargo build --release --package astroplant-mqtt-ingest

FROM debian:bullseye-slim

RUN apt-get update && apt-get install libpq5 -y
COPY --from=builder /usr/src/astroplant-api/target/release/astroplant-api /usr/local/bin/astroplant-api
COPY --from=builder /usr/src/astroplant-api/target/release/astroplant-mqtt-ingest /usr/local/bin/astroplant-mqtt-ingest
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
