CREATE TABLE media (
	id uuid NOT NULL,
	peripheral_id int4 NOT NULL,
	kit_id int4 NOT NULL,
	kit_configuration_id int4 NOT NULL,
	datetime timestamptz NOT NULL,
	"name" varchar NOT NULL,
	"type" varchar NOT NULL,
	metadata json NOT NULL,
	"size" int8 NOT NULL,
	CONSTRAINT media_pkey PRIMARY KEY (id),
	CONSTRAINT size_positive CHECK ((size >= 0))
);
CREATE INDEX ix_media_datetime ON public.media USING btree (datetime);
CREATE INDEX ix_media_kit_configuration_id ON public.media USING btree (kit_configuration_id);
CREATE INDEX ix_media_kit_id ON public.media USING btree (kit_id);
CREATE INDEX ix_media_peripheral_id ON public.media USING btree (peripheral_id);

-- foreign keys
ALTER TABLE public.media ADD CONSTRAINT media_kit_configuration_id_fkey FOREIGN KEY (kit_configuration_id) REFERENCES kit_configurations(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.media ADD CONSTRAINT media_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.media ADD CONSTRAINT media_peripheral_id_fkey FOREIGN KEY (peripheral_id) REFERENCES peripherals(id) ON DELETE CASCADE ON UPDATE CASCADE;
