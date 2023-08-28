CREATE TABLE kit_memberships (
	id serial4 NOT NULL,
	user_id int4 NOT NULL,
	kit_id int4 NOT NULL,
	datetime_linked timestamptz NOT NULL,
	access_super bool NOT NULL,
	access_configure bool NOT NULL,
	CONSTRAINT kit_memberships_pkey PRIMARY KEY (id)
);
CREATE INDEX ix_kit_memberships_kit_id ON public.kit_memberships USING btree (kit_id);
CREATE INDEX ix_kit_memberships_user_id ON public.kit_memberships USING btree (user_id);

-- foreign keys
ALTER TABLE public.kit_memberships ADD CONSTRAINT kit_memberships_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE public.kit_memberships ADD CONSTRAINT kit_memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;
