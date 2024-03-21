-- unfortunately this zombifies the column as pg doesn't truly remove
-- dropped columns... i can't find a way to simply alter the existing
-- column into a generated one in pg 14
ALTER TABLE kit_configurations
    DROP never_used,
    ADD never_used bool NOT NULL GENERATED ALWAYS AS (first_activated_at IS NULL) STORED;
