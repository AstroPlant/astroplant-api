CREATE TABLE peripheral_definition_expected_quantity_types (
	id serial4 NOT NULL,
	quantity_type_id int4 NOT NULL,
	peripheral_definition_id int4 NOT NULL,
	CONSTRAINT peripheral_definition_expected_quantity_types_pkey PRIMARY KEY (id)
);

-- foreign keys
ALTER TABLE public.peripheral_definition_expected_quantity_types ADD CONSTRAINT peripheral_definition_expected_qu_peripheral_definition_id_fkey FOREIGN KEY (peripheral_definition_id) REFERENCES peripheral_definitions(id);
ALTER TABLE public.peripheral_definition_expected_quantity_types ADD CONSTRAINT peripheral_definition_expected_quantity_t_quantity_type_id_fkey FOREIGN KEY (quantity_type_id) REFERENCES quantity_types(id);
