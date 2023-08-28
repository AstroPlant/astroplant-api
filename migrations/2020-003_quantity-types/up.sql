CREATE TABLE quantity_types (
	id serial4 NOT NULL,
	physical_quantity varchar(255) NOT NULL,
	physical_unit varchar(255) NOT NULL,
	physical_unit_symbol varchar(255) NULL,
	CONSTRAINT quantity_types_physical_quantity_physical_unit_key UNIQUE (physical_quantity, physical_unit),
	CONSTRAINT quantity_types_pkey PRIMARY KEY (id)
);
