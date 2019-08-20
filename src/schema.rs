table! {
    /// Representation of the `aggregate_measurements` table.
    ///
    /// (Automatically generated by Diesel.)
    aggregate_measurements (id) {
        /// The `id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `peripheral_id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_id -> Int4,
        /// The `kit_id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
        /// The `kit_configuration_id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_configuration_id -> Int4,
        /// The `quantity_type_id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        quantity_type_id -> Int4,
        /// The `aggregate_type` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        aggregate_type -> Varchar,
        /// The `value` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Float8`.
        ///
        /// (Automatically generated by Diesel.)
        value -> Float8,
        /// The `start_datetime` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        start_datetime -> Timestamptz,
        /// The `end_datetime` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        end_datetime -> Timestamptz,
    }
}

table! {
    /// Representation of the `kit_configurations` table.
    ///
    /// (Automatically generated by Diesel.)
    kit_configurations (id) {
        /// The `id` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `kit_id` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
    }
}

table! {
    /// Representation of the `kit_memberships` table.
    ///
    /// (Automatically generated by Diesel.)
    kit_memberships (id) {
        /// The `id` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `user_id` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        user_id -> Int4,
        /// The `kit_id` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
        /// The `datetime_linked` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        datetime_linked -> Timestamptz,
        /// The `access_super` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        access_super -> Bool,
        /// The `access_configure` column of the `kit_memberships` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        access_configure -> Bool,
    }
}

table! {
    /// Representation of the `kits` table.
    ///
    /// (Automatically generated by Diesel.)
    kits (id) {
        /// The `id` column of the `kits` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `serial` column of the `kits` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        serial -> Varchar,
        /// The `password_hash` column of the `kits` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        password_hash -> Varchar,
        /// The `name` column of the `kits` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Nullable<Varchar>,
        /// The `description` column of the `kits` table.
        ///
        /// Its SQL type is `Nullable<Text>`.
        ///
        /// (Automatically generated by Diesel.)
        description -> Nullable<Text>,
        /// The `latitude` column of the `kits` table.
        ///
        /// Its SQL type is `Nullable<Numeric>`.
        ///
        /// (Automatically generated by Diesel.)
        latitude -> Nullable<Numeric>,
        /// The `longitude` column of the `kits` table.
        ///
        /// Its SQL type is `Nullable<Numeric>`.
        ///
        /// (Automatically generated by Diesel.)
        longitude -> Nullable<Numeric>,
        /// The `privacy_public_dashboard` column of the `kits` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        privacy_public_dashboard -> Bool,
        /// The `privacy_show_on_map` column of the `kits` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        privacy_show_on_map -> Bool,
    }
}

table! {
    /// Representation of the `peripheral_definition_expected_quantity_type` table.
    ///
    /// (Automatically generated by Diesel.)
    peripheral_definition_expected_quantity_type (id) {
        /// The `id` column of the `peripheral_definition_expected_quantity_type` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `quantity_type_id` column of the `peripheral_definition_expected_quantity_type` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        quantity_type_id -> Int4,
        /// The `peripheral_definition_id` column of the `peripheral_definition_expected_quantity_type` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_definition_id -> Int4,
    }
}

table! {
    /// Representation of the `peripheral_definitions` table.
    ///
    /// (Automatically generated by Diesel.)
    peripheral_definitions (id) {
        /// The `id` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `name` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Varchar,
        /// The `description` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Nullable<Text>`.
        ///
        /// (Automatically generated by Diesel.)
        description -> Nullable<Text>,
        /// The `brand` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        brand -> Nullable<Varchar>,
        /// The `model` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        model -> Nullable<Varchar>,
        /// The `module_name` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        module_name -> Varchar,
        /// The `class_name` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        class_name -> Varchar,
        /// The `configuration_schema` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        configuration_schema -> Json,
    }
}

table! {
    /// Representation of the `peripherals` table.
    ///
    /// (Automatically generated by Diesel.)
    peripherals (id) {
        /// The `id` column of the `peripherals` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `kit_id` column of the `peripherals` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
        /// The `kit_configuration_id` column of the `peripherals` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_configuration_id -> Int4,
        /// The `peripheral_definition_id` column of the `peripherals` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_definition_id -> Int4,
        /// The `name` column of the `peripherals` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Varchar,
        /// The `configuration` column of the `peripherals` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        configuration -> Json,
    }
}

table! {
    /// Representation of the `quantity_types` table.
    ///
    /// (Automatically generated by Diesel.)
    quantity_types (id) {
        /// The `id` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `physical_quantity` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        physical_quantity -> Varchar,
        /// The `physical_unit` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        physical_unit -> Varchar,
        /// The `physical_unit_symbol` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        physical_unit_symbol -> Nullable<Varchar>,
    }
}

table! {
    /// Representation of the `raw_measurements` table.
    ///
    /// (Automatically generated by Diesel.)
    raw_measurements (id) {
        /// The `id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `peripheral_id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_id -> Int4,
        /// The `kit_id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
        /// The `kit_configuration_id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_configuration_id -> Int4,
        /// The `quantity_type_id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        quantity_type_id -> Int4,
        /// The `value` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Float8`.
        ///
        /// (Automatically generated by Diesel.)
        value -> Float8,
        /// The `datetime` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        datetime -> Timestamptz,
    }
}

table! {
    /// Representation of the `users` table.
    ///
    /// (Automatically generated by Diesel.)
    users (id) {
        /// The `id` column of the `users` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `username` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        username -> Varchar,
        /// The `display_name` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        display_name -> Varchar,
        /// The `password_hash` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        password_hash -> Varchar,
        /// The `email_address` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        email_address -> Varchar,
        /// The `use_gravatar` column of the `users` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        use_gravatar -> Bool,
        /// The `gravatar_alternative` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        gravatar_alternative -> Varchar,
    }
}

joinable!(aggregate_measurements -> kit_configurations (kit_configuration_id));
joinable!(aggregate_measurements -> kits (kit_id));
joinable!(aggregate_measurements -> peripherals (peripheral_id));
joinable!(aggregate_measurements -> quantity_types (quantity_type_id));
joinable!(kit_configurations -> kits (kit_id));
joinable!(kit_memberships -> kits (kit_id));
joinable!(kit_memberships -> users (user_id));
joinable!(peripheral_definition_expected_quantity_type -> peripheral_definitions (peripheral_definition_id));
joinable!(peripheral_definition_expected_quantity_type -> quantity_types (quantity_type_id));
joinable!(peripherals -> kit_configurations (kit_configuration_id));
joinable!(peripherals -> kits (kit_id));
joinable!(peripherals -> peripheral_definitions (peripheral_definition_id));
joinable!(raw_measurements -> kit_configurations (kit_configuration_id));
joinable!(raw_measurements -> kits (kit_id));
joinable!(raw_measurements -> peripherals (peripheral_id));
joinable!(raw_measurements -> quantity_types (quantity_type_id));

allow_tables_to_appear_in_same_query!(
    aggregate_measurements,
    kit_configurations,
    kit_memberships,
    kits,
    peripheral_definition_expected_quantity_type,
    peripheral_definitions,
    peripherals,
    quantity_types,
    raw_measurements,
    users,
);
