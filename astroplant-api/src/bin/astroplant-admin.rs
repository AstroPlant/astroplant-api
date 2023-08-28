use clap::Parser;

use astroplant_api::admin::{
    self, insert_astroplant_definitions::ExistingPeripheralDefinitionStrategy, migrate,
};

/// AstroPlant backend administration tools.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Command {
    /// Migrate the database schema
    Migrate,
    /// Insert the AstroPlant-specific data definitions into the database. This includes quantity
    /// types and peripheral definitions.
    InsertAstroplantDefinitions(InsertAstroplantDefinitionsOpts),
}

#[derive(Parser, Debug)]
struct InsertAstroplantDefinitionsOpts {
    /// Insert definitions for virtual AstroPlant sensors and actuators. These can be used for
    /// development purposes.
    #[clap(long)]
    simulation_definitions: bool,
    #[clap(long)]
    update_existing_peripheral_definitions: bool,
}

fn main() -> anyhow::Result<()> {
    astroplant_api::utils::tracing::init();
    let command = Command::parse();

    tracing::debug!("Connecting to database");
    let mut conn = astroplant_api::database::oneoff_connection()?;
    tracing::debug!("Connected to database");

    match command {
        Command::Migrate => {
            migrate::run(&mut conn)?;
        }
        Command::InsertAstroplantDefinitions(opts) => {
            let existing_peripheral_definition_strategy =
                if opts.update_existing_peripheral_definitions {
                    ExistingPeripheralDefinitionStrategy::Update
                } else {
                    ExistingPeripheralDefinitionStrategy::Skip
                };

            admin::insert_astroplant_definitions(
                &mut conn,
                opts.simulation_definitions.into(),
                existing_peripheral_definition_strategy,
            )?;
        }
    }

    Ok(())
}
