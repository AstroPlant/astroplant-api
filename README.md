# AstroPlant API

This is the main AstroPlant API, providing access to an AstroPlant backend over HTTP.

## Building with Cargo

Run:

```shell
$ cargo build --release
```

The executable will be built to `target/release/astroplant-rs-api`.

## Configuration

This application requires a secret key for signing and verifying authentication tokens.
The key must be provided in a file (as raw bytes).

By default, this application reads the key from `./token_signer.key` relative to the working directory.
A different file can be used by naming it in the `TOKEN_SIGNER_KEY` environment variable.

Set environment variables to configure the program.

| Variable | Description | Default |
|-|-|-|
| `DATABASE_URL` | The database connection url. | `postgres://astroplant:astroplant@localhost/astroplant` |
| `MQTT_HOST` | The hostname of the MQTT broker. | `mqtt.ops` |
| `MQTT_PORT` | The port of the MQTT broker. | `1883` |
| `MQTT_USERNAME` | The username for MQTT authentication. | `server` |
| `MQTT_PASSWORD` | The password for MQTT authentication. | |
