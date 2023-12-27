CREATE TABLE queue_media_pending_deletion (
    id serial PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    media_id uuid NOT NULL UNIQUE,
    media_datetime timestamptz NOT NULL,
    media_size int8 NOT NULL,
    CONSTRAINT media_size_positive CHECK ((media_size >= 0))
);
