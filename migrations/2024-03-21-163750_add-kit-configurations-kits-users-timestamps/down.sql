DROP TRIGGER kit_configurations_90_updated_at ON kit_configurations;

DROP TRIGGER kits_90_updated_at ON kits;

DROP TRIGGER users_90_updated_at ON users;

DROP FUNCTION bump_updated_at;

ALTER TABLE kit_configurations
    DROP COLUMN updated_at,
    DROP COLUMN created_at;

ALTER TABLE kits
    DROP COLUMN updated_at,
    DROP COLUMN created_at;

ALTER TABLE users
    DROP COLUMN updated_at,
    DROP COLUMN created_at
