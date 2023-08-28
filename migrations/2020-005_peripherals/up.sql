CREATE TABLE peripherals (
	id serial4 NOT NULL,
	kit_id int4 NOT NULL,
	kit_configuration_id int4 NOT NULL,
	peripheral_definition_id int4 NOT NULL,
	"name" varchar(255) NOT NULL,
	"configuration" json NOT NULL,
	CONSTRAINT peripherals_pkey PRIMARY KEY (id)
);
CREATE INDEX ix_peripherals_kit_id ON public.peripherals USING btree (kit_id);

-- foreign keys
ALTER TABLE public.peripherals ADD CONSTRAINT peripherals_kit_configuration_id_fkey FOREIGN KEY (kit_configuration_id) REFERENCES kit_configurations(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.peripherals ADD CONSTRAINT peripherals_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.peripherals ADD CONSTRAINT peripherals_peripheral_definition_id_fkey FOREIGN KEY (peripheral_definition_id) REFERENCES peripheral_definitions(id) ON DELETE CASCADE ON UPDATE CASCADE;
