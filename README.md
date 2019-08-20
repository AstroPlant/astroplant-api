# AstroPlant API

This is the main AstroPlant API, providing access to an AstroPlant backend over HTTP.

## Configuration

This application requires a secret key for signing and verifying authentication tokens.
The key must be provided in a file (as raw bytes).

By default, this application reads the key from `./token_signer.key` relative to the working directory.
A different file can be used by naming it in the `TOKEN_SIGNER_KEY` environment variable.
