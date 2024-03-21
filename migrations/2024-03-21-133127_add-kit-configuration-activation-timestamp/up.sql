CREATE FUNCTION kit_configurations_set_first_activated_at ()
    RETURNS TRIGGER
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.active AND NEW.first_activated_at IS NULL THEN
        NEW.first_activated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$;

ALTER TABLE kit_configurations
    ADD first_activated_at timestamptz;

CREATE TRIGGER kit_configurations_10_set_first_activated_at
    BEFORE UPDATE ON kit_configurations
    FOR EACH ROW
    EXECUTE FUNCTION kit_configurations_set_first_activated_at ();

-- Set kit_configurations.first_activated_at for all configurations that are not `never_used`.
UPDATE
    kit_configurations kc
SET
    first_activated_at = now()
WHERE
    NOT kc.never_used;

-- As an estimation, set kit_configurations.first_activated_at to the date and
-- time of the earliest known measurement, if any.
UPDATE
    kit_configurations kc
SET
    first_activated_at = m.datetime_earliest
FROM (
    SELECT
        kit_configuration_id,
        min(datetime) AS datetime_earliest
    FROM
        raw_measurements
    GROUP BY
        kit_configuration_id) AS m
WHERE
    NOT kc.never_used
    AND m.kit_configuration_id = kc.id;
