mod config;
mod magnifier;
mod monitor;
mod layer_surface;
mod pool_buffer;
mod renderer;
mod input;
mod protocols;
mod utils;

use clap::Parser;
use config::{Cli, Config};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    env_logger::Builder::new()
        .filter_level(if cli.quiet {
            log::LevelFilter::Error
        } else if cli.verbose {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        })
        .init();

    log::info!("Starting remagnify v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::from_cli(cli);
    log::debug!("Configuration: {:?}", config);

    let mut magnifier = magnifier::Magnifier::new(config)?;
    magnifier.run()?;

    log::info!("Exiting remagnify");
    Ok(())
}
