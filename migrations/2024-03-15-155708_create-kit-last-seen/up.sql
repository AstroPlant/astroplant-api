CREATE TABLE kit_last_seen (
    kit_id int4 NOT NULL,
    datetime_last_seen timestamptz NOT NULL,
    CONSTRAINT kit_last_seen_pkey PRIMARY KEY (kit_id)
);

CREATE INDEX ix_kit_last_seen_datetime_last_seen ON public.kit_last_seen USING btree (datetime_last_seen);

-- foreign keys
ALTER TABLE public.kit_last_seen
    ADD CONSTRAINT kit_last_seen_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits (id) ON DELETE CASCADE ON UPDATE CASCADE
