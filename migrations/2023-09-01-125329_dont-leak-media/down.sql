ALTER TABLE media
    DROP CONSTRAINT media_peripheral_id_fkey,
    DROP CONSTRAINT media_kit_id_fkey,
    DROP CONSTRAINT media_kit_configuration_id_fkey;

ALTER TABLE media
    ADD CONSTRAINT media_peripheral_id_fkey FOREIGN KEY (peripheral_id) REFERENCES peripherals (id) ON DELETE CASCADE ON UPDATE CASCADE,
    ADD CONSTRAINT media_kit_id_fkey FOREIGN KEY (kit_id) REFERENCES kits (id) ON DELETE CASCADE ON UPDATE CASCADE,
    ADD CONSTRAINT media_kit_configuration_id_fkey FOREIGN KEY (kit_configuration_id) REFERENCES kit_configurations (id) ON DELETE CASCADE ON UPDATE CASCADE
