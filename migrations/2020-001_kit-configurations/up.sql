CREATE TABLE kit_configurations (
	id serial4 NOT NULL,
	kit_id int4 NOT NULL,
	description text NULL,
	controller_symbol_location text NOT NULL,
	controller_symbol text NOT NULL,
	control_rules json NOT NULL,
	active bool NOT NULL DEFAULT false,
	never_used bool NOT NULL DEFAULT true,
	CONSTRAINT active_and_never_used CHECK ((NOT (active AND never_used))),
	CONSTRAINT kit_configurations_pkey PRIMARY KEY (id)
);
CREATE INDEX ix_kit_configurations_active ON public.kit_configurations USING btree (active);

-- foreign keys
ALTER TABLE public.kit_configurations ADD CONSTRAINT kit_configurations_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits(id) ON DELETE CASCADE ON UPDATE CASCADE;
