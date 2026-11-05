mod cli;
mod headless;
mod scenario;

use cli::Args;
use flair_core::{config::Config, link::channel_pair};
use flair_server::{ServerConfig, runtime::spawn_in_process};
use std::{
    env,
    process::ExitCode,
    time::{Duration, Instant},
};

fn main() -> ExitCode {
    let start = Instant::now();
    env_logger::init();
    let args = match Args::parse_from(env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("flair: {error}");
            return ExitCode::from(2);
        }
    };
    match args {
        Args::Help => {
            println!(
                "flair [--config PATH] [--standalone]\nflair test-headless --scenario PATH --output DIR [--timeout-ms 5000]\n\nM0 runs one in-process terminal. Headless tests open no desktop window."
            );
            ExitCode::SUCCESS
        }
        Args::Version => {
            println!("flair {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Args::Headless {
            scenario,
            output,
            timeout_ms,
        } => {
            let scenario = match std::fs::read_to_string(&scenario)
                .map_err(|e| e.to_string())
                .and_then(|text| scenario::Scenario::parse(&text))
            {
                Ok(scenario) => scenario,
                Err(error) => {
                    eprintln!("flair: {error}");
                    return ExitCode::from(2);
                }
            };
            match headless::run(scenario, &output, Duration::from_millis(timeout_ms)) {
                Ok(()) => {
                    println!("headless: passed; artifacts {}", output.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("flair: {error}; artifacts {}", output.display());
                    ExitCode::FAILURE
                }
            }
        }
        Args::Gui { config: path } => {
            let (config, error) = Config::load(&path.unwrap_or_else(Config::default_path));
            if let Some(error) = error {
                log::warn!("{error}; using defaults");
            }
            if env::var_os("FLAIR_STARTUP_PROBE").is_some() {
                eprintln!("probe config {:.1}", start.elapsed().as_secs_f64() * 1000.0);
            }
            let (client, server) = channel_pair();
            let worker = match spawn_in_process(ServerConfig::from_config(&config), server) {
                Ok(worker) => worker,
                Err(error) => {
                    eprintln!("flair: {error}");
                    return ExitCode::FAILURE;
                }
            };
            let result = flair_gui::run(
                config,
                client,
                env::var_os("FLAIR_STARTUP_PROBE").map(|_| start),
            );
            if worker.join().is_err() {
                eprintln!("flair: server thread panicked");
                return ExitCode::FAILURE;
            }
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("flair: {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
