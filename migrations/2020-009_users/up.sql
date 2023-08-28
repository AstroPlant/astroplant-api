CREATE TABLE users (
	id serial4 NOT NULL,
	username varchar(40) NOT NULL,
	display_name varchar(40) NOT NULL,
	password_hash varchar(255) NOT NULL,
	email_address varchar(255) NOT NULL,
	use_email_address_for_gravatar bool NOT NULL DEFAULT true,
	gravatar_alternative varchar(255) NOT NULL,
	CONSTRAINT users_pkey PRIMARY KEY (id)
);
CREATE UNIQUE INDEX ix_users_email_address ON public.users USING btree (email_address);
CREATE UNIQUE INDEX ix_users_username ON public.users USING btree (username);
