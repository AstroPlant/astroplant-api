CREATE FUNCTION bump_updated_at ()
    RETURNS TRIGGER
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

ALTER TABLE kit_configurations
    ADD created_at timestamptz NOT NULL DEFAULT now(),
    ADD updated_at timestamptz NOT NULL DEFAULT now();

ALTER TABLE kits
    ADD created_at timestamptz NOT NULL DEFAULT now(),
    ADD updated_at timestamptz NOT NULL DEFAULT now();

ALTER TABLE users
    ADD created_at timestamptz NOT NULL DEFAULT now(),
    ADD updated_at timestamptz NOT NULL DEFAULT now();

CREATE TRIGGER kit_configurations_90_updated_at
    BEFORE UPDATE ON kit_configurations
    FOR EACH ROW
    EXECUTE FUNCTION bump_updated_at ();

CREATE TRIGGER kits_90_updated_at
    BEFORE UPDATE ON kits
    FOR EACH ROW
    EXECUTE FUNCTION bump_updated_at ();

CREATE TRIGGER users_90_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION bump_updated_at ();

-- As an estimation, set kit_configurations.created_at to the earliest
-- first_activated_at date and time. A previous migration sets this to the
-- earliest known measurement, if any.
UPDATE
    kit_configurations kc
SET
    created_at = kc.first_activated_at
WHERE
    kc.first_activated_at IS NOT NULL;

-- As an estimation, set kits.created_at to the earliest
-- kit_configurations.created_at of that kit.
UPDATE
    kits
SET
    created_at = kc.created_at_earliest
FROM (
    SELECT
        kit_id,
        min(created_at) AS created_at_earliest
    FROM
        kit_configurations
    GROUP BY
        kit_id) AS kc
WHERE
    kc.kit_id = id;

-- As an estimation, set users.created_at to the earliest kits.created_at of
-- kits they are a member of, if any.
UPDATE
    users
SET
    created_at = m.datetime_earliest
FROM (
    SELECT
        user_id,
        min(created_at) AS datetime_earliest
    FROM
        kits k
        INNER JOIN kit_memberships km ON k.id = km.kit_id
    GROUP BY
        user_id) AS m
WHERE
    m.user_id = id;
