CREATE UNIQUE INDEX ix_kit_configurations_active_kit_id ON public.kit_configurations (kit_id)
WHERE (active);

COMMENT ON INDEX ix_kit_configurations_active_kit_id IS 'At most one kit configuration may be active per kit at any one time.'
