# AstroPlant Object Storage
This crate interfaces with an object storage.
It implements an interface for an S3-style API or the local filesystem.

When using the S3 interface, the region name and S3 endpoint URL must be provided.
The following S3 credentials can be given as environment variables:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_SESSION_TOKEN`
- `AWS_CREDENTIAL_EXPIRATION`
