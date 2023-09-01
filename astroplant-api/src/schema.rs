// @generated automatically by Diesel CLI.

diesel::table! {
    /// Representation of the `aggregate_measurements` table.
    ///
    /// (Automatically generated by Diesel.)
    aggregate_measurements (id) {
        /// The `id` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Uuid`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Uuid,
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
        /// The `datetime_start` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        datetime_start -> Timestamptz,
        /// The `datetime_end` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        datetime_end -> Timestamptz,
        /// The `values` column of the `aggregate_measurements` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        values -> Json,
    }
}

diesel::table! {
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
        /// The `description` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Nullable<Text>`.
        ///
        /// (Automatically generated by Diesel.)
        description -> Nullable<Text>,
        /// The `controller_symbol_location` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        controller_symbol_location -> Text,
        /// The `controller_symbol` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        controller_symbol -> Text,
        /// The `control_rules` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        control_rules -> Json,
        /// The `active` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        active -> Bool,
        /// The `never_used` column of the `kit_configurations` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        never_used -> Bool,
    }
}

diesel::table! {
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

diesel::table! {
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
        #[max_length = 20]
        serial -> Varchar,
        /// The `password_hash` column of the `kits` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        password_hash -> Varchar,
        /// The `name` column of the `kits` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
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

diesel::table! {
    /// Representation of the `media` table.
    ///
    /// (Automatically generated by Diesel.)
    media (id) {
        /// The `id` column of the `media` table.
        ///
        /// Its SQL type is `Uuid`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Uuid,
        /// The `peripheral_id` column of the `media` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_id -> Int4,
        /// The `kit_id` column of the `media` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_id -> Int4,
        /// The `kit_configuration_id` column of the `media` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        kit_configuration_id -> Int4,
        /// The `datetime` column of the `media` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        datetime -> Timestamptz,
        /// The `name` column of the `media` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Varchar,
        /// The `type` column of the `media` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[sql_name = "type"]
        type_ -> Varchar,
        /// The `metadata` column of the `media` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        metadata -> Json,
        /// The `size` column of the `media` table.
        ///
        /// Its SQL type is `Int8`.
        ///
        /// (Automatically generated by Diesel.)
        size -> Int8,
    }
}

diesel::table! {
    /// Representation of the `peripheral_definition_expected_quantity_types` table.
    ///
    /// (Automatically generated by Diesel.)
    peripheral_definition_expected_quantity_types (id) {
        /// The `id` column of the `peripheral_definition_expected_quantity_types` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `quantity_type_id` column of the `peripheral_definition_expected_quantity_types` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        quantity_type_id -> Int4,
        /// The `peripheral_definition_id` column of the `peripheral_definition_expected_quantity_types` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        peripheral_definition_id -> Int4,
    }
}

diesel::table! {
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
        #[max_length = 100]
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
        #[max_length = 100]
        brand -> Nullable<Varchar>,
        /// The `model` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 100]
        model -> Nullable<Varchar>,
        /// The `symbol_location` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        symbol_location -> Varchar,
        /// The `symbol` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        symbol -> Varchar,
        /// The `configuration_schema` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        configuration_schema -> Json,
        /// The `command_schema` column of the `peripheral_definitions` table.
        ///
        /// Its SQL type is `Nullable<Json>`.
        ///
        /// (Automatically generated by Diesel.)
        command_schema -> Nullable<Json>,
    }
}

diesel::table! {
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
        #[max_length = 255]
        name -> Varchar,
        /// The `configuration` column of the `peripherals` table.
        ///
        /// Its SQL type is `Json`.
        ///
        /// (Automatically generated by Diesel.)
        configuration -> Json,
    }
}

diesel::table! {
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
        #[max_length = 255]
        physical_quantity -> Varchar,
        /// The `physical_unit` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        physical_unit -> Varchar,
        /// The `physical_unit_symbol` column of the `quantity_types` table.
        ///
        /// Its SQL type is `Nullable<Varchar>`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        physical_unit_symbol -> Nullable<Varchar>,
    }
}

diesel::table! {
    /// Representation of the `raw_measurements` table.
    ///
    /// (Automatically generated by Diesel.)
    raw_measurements (id) {
        /// The `id` column of the `raw_measurements` table.
        ///
        /// Its SQL type is `Uuid`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Uuid,
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

diesel::table! {
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
        #[max_length = 40]
        username -> Varchar,
        /// The `display_name` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 40]
        display_name -> Varchar,
        /// The `password_hash` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        password_hash -> Varchar,
        /// The `email_address` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        email_address -> Varchar,
        /// The `use_email_address_for_gravatar` column of the `users` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        use_email_address_for_gravatar -> Bool,
        /// The `gravatar_alternative` column of the `users` table.
        ///
        /// Its SQL type is `Varchar`.
        ///
        /// (Automatically generated by Diesel.)
        #[max_length = 255]
        gravatar_alternative -> Varchar,
    }
}

diesel::joinable!(aggregate_measurements -> kit_configurations (kit_configuration_id));
diesel::joinable!(aggregate_measurements -> kits (kit_id));
diesel::joinable!(aggregate_measurements -> peripherals (peripheral_id));
diesel::joinable!(aggregate_measurements -> quantity_types (quantity_type_id));
diesel::joinable!(kit_configurations -> kits (kit_id));
diesel::joinable!(kit_memberships -> kits (kit_id));
diesel::joinable!(kit_memberships -> users (user_id));
diesel::joinable!(media -> kit_configurations (kit_configuration_id));
diesel::joinable!(media -> kits (kit_id));
diesel::joinable!(media -> peripherals (peripheral_id));
diesel::joinable!(peripheral_definition_expected_quantity_types -> peripheral_definitions (peripheral_definition_id));
diesel::joinable!(peripheral_definition_expected_quantity_types -> quantity_types (quantity_type_id));
diesel::joinable!(peripherals -> kit_configurations (kit_configuration_id));
diesel::joinable!(peripherals -> kits (kit_id));
diesel::joinable!(peripherals -> peripheral_definitions (peripheral_definition_id));
diesel::joinable!(raw_measurements -> kit_configurations (kit_configuration_id));
diesel::joinable!(raw_measurements -> kits (kit_id));
diesel::joinable!(raw_measurements -> peripherals (peripheral_id));
diesel::joinable!(raw_measurements -> quantity_types (quantity_type_id));

diesel::allow_tables_to_appear_in_same_query!(
    aggregate_measurements,
    kit_configurations,
    kit_memberships,
    kits,
    media,
    peripheral_definition_expected_quantity_types,
    peripheral_definitions,
    peripherals,
    quantity_types,
    raw_measurements,
    users,
);
