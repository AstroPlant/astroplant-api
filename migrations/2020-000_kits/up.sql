CREATE TABLE kits (
	id serial4 NOT NULL,
	serial varchar(20) NOT NULL,
	password_hash varchar(255) NOT NULL,
	name varchar(255) NULL,
	description text NULL,
	latitude numeric(11, 8) NULL,
	longitude numeric(11, 8) NULL,
	privacy_public_dashboard bool NOT NULL DEFAULT false,
	privacy_show_on_map bool NOT NULL DEFAULT false,
	CONSTRAINT kits_pkey PRIMARY KEY (id)
);
CREATE INDEX ix_kits_privacy_show_on_map ON public.kits USING btree (privacy_show_on_map);
CREATE UNIQUE INDEX ix_kits_serial ON public.kits USING btree (serial);
