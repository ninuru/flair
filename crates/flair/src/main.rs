mod cli;
mod daemon;
mod headless;
mod scenario;

use cli::Args;
use flair_core::{
    config::Config,
    link::channel_pair,
    proto::{Control, Target},
};
use flair_server::{ServerConfig, runtime::spawn_in_process};
use std::{
    env,
    process::ExitCode,
    time::{Duration, Instant},
};

fn main() -> ExitCode {
    let start = Instant::now();

    let args = match Args::parse_from(env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("flair: {error}");
            return ExitCode::from(2);
        }
    };
    let mut logger = env_logger::Builder::from_env(env_logger::Env::default());
    if matches!(args, Args::Server { .. }) {
        match daemon::log_file() {
            Ok(file) => {
                logger.target(env_logger::Target::Pipe(Box::new(file)));
            }
            Err(error) => {
                eprintln!("flair: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    logger.init();
    match args {
        Args::Help => {
            println!(
                "flair [--config PATH] [--standalone]\nflair server [--config PATH]\nflair new [-d] [-s NAME] [-c DIR]\nflair attach -t NAME\nflair switch PATH|NAME\nflair ls\nflair kill-server\nflair test-headless --scenario PATH --output DIR [--timeout-ms 5000]"
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
        Args::Server { config: path } => {
            let (config, error) = Config::load(&path.unwrap_or_else(Config::default_path));
            if let Some(error) = error {
                log::warn!("{error}; using defaults");
            }
            finish((|| {
                let path = flair_server::socket::socket_path()?;
                let config = daemon::server_config(&config)?;
                flair_server::runtime::serve(config, &path)
            })())
        }
        Args::KillServer => finish(daemon::kill()),
        Args::List => finish((|| {
            let link = daemon::connect(None, false)?;
            for session in daemon::control(&link, Control::List)? {
                println!(
                    "{}: {} window(s) {}",
                    session.name,
                    session.windows,
                    session.root.display()
                );
            }
            Ok(())
        })()),
        Args::Create { config, target } => finish((|| {
            let link = daemon::connect(config.as_deref(), true)?;
            let sessions = daemon::control(
                &link,
                Control::Create {
                    target,
                    env: daemon::environment(),
                },
            )?;
            for session in sessions {
                println!("{}", session.name);
            }
            Ok(())
        })()),
        Args::Switch(target) => finish((|| {
            let link = daemon::connect(None, true)?;
            let pane = daemon::pane_for_socket(&flair_server::socket::socket_path()?);
            let sessions = daemon::control(
                &link,
                Control::Switch {
                    target,
                    pane,
                    env: daemon::environment(),
                },
            )?;
            if let Some(session) = sessions.first() {
                drop(link);
                let (config, _) = Config::load(&Config::default_path());
                let link = daemon::connect(None, true)?;
                flair_gui::run_target(
                    config,
                    link,
                    None,
                    Target::Name(session.name.clone()),
                    daemon::environment(),
                )
                .map_err(std::io::Error::other)?;
            }
            Ok(())
        })()),
        Args::Gui {
            config: path,
            target,
            standalone,
        } => {
            let (config, error) = Config::load(&path.clone().unwrap_or_else(Config::default_path));
            if let Some(error) = error {
                log::warn!("{error}; using defaults");
            }
            if env::var_os("FLAIR_STARTUP_PROBE").is_some() {
                eprintln!("probe config {:.1}", start.elapsed().as_secs_f64() * 1000.0);
            }
            let (client, worker) = if standalone {
                let (client, server) = channel_pair();
                match spawn_in_process(ServerConfig::from_config(&config), server) {
                    Ok(worker) => (client, Some(worker)),
                    Err(error) => {
                        eprintln!("flair: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                match daemon::connect(path.as_deref(), true) {
                    Ok(client) => (client, None),
                    Err(error) => {
                        eprintln!("flair: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            };
            let result = flair_gui::run_target(
                config,
                client,
                env::var_os("FLAIR_STARTUP_PROBE").map(|_| start),
                target,
                daemon::environment(),
            );
            if let Some(worker) = worker
                && worker.join().is_err()
            {
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

fn finish(result: std::io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("flair: {error}");
            ExitCode::FAILURE
        }
    }
}
