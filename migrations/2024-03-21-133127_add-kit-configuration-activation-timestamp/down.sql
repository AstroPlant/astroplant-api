DROP TRIGGER kit_configurations_10_set_first_activated_at ON kit_configurations;

DROP FUNCTION kit_configurations_set_first_activated_at;

ALTER TABLE kit_configurations
    DROP COLUMN first_activated_at
