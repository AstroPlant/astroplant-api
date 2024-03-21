INSERT INTO kit_last_seen
SELECT
    kit_id,
    MAX(datetime_start) AS datetime_last_seen
FROM
    aggregate_measurements
GROUP BY
    kit_id
ON CONFLICT
    DO NOTHING
