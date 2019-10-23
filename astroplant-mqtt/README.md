# AstroPlant MQTT API
This is an implementation of the back-end side of the MQTT API.
This implementation assumes the MQTT broker handles authentication and authorization of all MQTT subscribers and publishers.

## Protocol
There are six MQTT topics:

| Topic | Description |
| ----- | ----------- |
| `kit/{kitSerial}/measurement/raw` | Kits' raw, real-time measurements |
| `kit/{kitSerial}/measurement/aggregate` | Kits' aggregated measurements  |
| `kit/{kitSerial}/server-rpc/request` | RPC requests from the kit to the server. |
| `kit/{kitSerial}/server-rpc/response` | RPC responses from the server. |
| `kit/{kitSerial}/kit-rpc/request` | RPC request from the server to the kit. |
| `kit/{kitSerial}/kit-rpc/response` | RPC responses from the kit. |

The messages sent through these topics are serialized through Cap'n Proto.
The Cap'n Proto schema is defined in `./proto/astroplant.capnp`.

Each RPC request contains an `id` field.
RPC responses echo the provided `id` to allow clients to match responses with requests.
Note this RPC protocol is intended for 1-to-1 communication through MQTT.

## Server RPC
The server RPC supports the following methods:

| Method | Description |
| ------ | ----------- |
| `version` | Get the version of the server. |
| `getActiveConfiguration` | Get the active configuration of the kit. |

## Kit RPC
The kit RPC supporst the following methods:

| Method | Description |
| ------ | ----------- |
| `version` | Get the version of the kit. |
| `uptime` | Get the amount of time in seconds the kit has been up without interruption. |
