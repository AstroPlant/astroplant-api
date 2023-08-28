CREATE TABLE peripheral_definitions (
	id serial4 NOT NULL,
	"name" varchar(100) NOT NULL,
	description text NULL,
	brand varchar(100) NULL,
	model varchar(100) NULL,
	symbol_location varchar(255) NOT NULL,
	symbol varchar(255) NOT NULL,
	configuration_schema json NOT NULL,
	command_schema json NULL,
	CONSTRAINT peripheral_definitions_name_key UNIQUE (name),
	CONSTRAINT peripheral_definitions_pkey PRIMARY KEY (id)
);
