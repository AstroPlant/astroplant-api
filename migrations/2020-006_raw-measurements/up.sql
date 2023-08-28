CREATE TABLE raw_measurements (
	id uuid NOT NULL,
	peripheral_id int4 NOT NULL,
	kit_id int4 NOT NULL,
	kit_configuration_id int4 NOT NULL,
	quantity_type_id int4 NOT NULL,
	value float8 NOT NULL,
	datetime timestamptz NOT NULL,
	CONSTRAINT raw_measurements_pkey PRIMARY KEY (id)
);
CREATE INDEX ix_raw_measurements_datetime ON public.raw_measurements USING btree (datetime);
CREATE INDEX ix_raw_measurements_kit_configuration_id ON public.raw_measurements USING btree (kit_configuration_id);
CREATE INDEX ix_raw_measurements_kit_id ON public.raw_measurements USING btree (kit_id);
CREATE INDEX ix_raw_measurements_peripheral_id ON public.raw_measurements USING btree (peripheral_id);
CREATE INDEX ix_raw_measurements_quantity_type_id ON public.raw_measurements USING btree (quantity_type_id);

-- foreign keys
ALTER TABLE public.raw_measurements ADD CONSTRAINT raw_measurements_kit_configuration_id_fkey FOREIGN KEY (kit_configuration_id) REFERENCES kit_configurations(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.raw_measurements ADD CONSTRAINT raw_measurements_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.raw_measurements ADD CONSTRAINT raw_measurements_peripheral_id_fkey FOREIGN KEY (peripheral_id) REFERENCES peripherals(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.raw_measurements ADD CONSTRAINT raw_measurements_quantity_type_id_fkey FOREIGN KEY (quantity_type_id) REFERENCES quantity_types(id) ON DELETE CASCADE ON UPDATE CASCADE;
