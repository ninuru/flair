mod cli;

use std::{env, process::ExitCode, time::Instant};

use flair_core::{config::Config, link::channel_pair};
use flair_server::{ServerConfig, runtime::spawn_in_process};

fn main() -> ExitCode {
    let start = Instant::now();
    let config = Config::default();
    let (client, server) = channel_pair();
    if let Err(error) = spawn_in_process(ServerConfig::from_config(&config), server) {
        eprintln!("flair: {error}");
        return ExitCode::FAILURE;
    }
    match flair_gui::run(
        config,
        client,
        env::var_os("FLAIR_STARTUP_PROBE").map(|_| start),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("flair: {error}");
            ExitCode::FAILURE
        }
    }
}
