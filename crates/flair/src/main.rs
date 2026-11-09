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
use flair_gui::drive::Drive;
use flair_server::{ServerConfig, runtime::spawn_in_process};
use scenario::{Scenario, Step, key_input};
use std::{
    env,
    path::Path,
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
    let mut logger = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("error,flair=warn"),
    );
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
                "flair [--config PATH] [--standalone]\nflair server [--config PATH]\nflair new [-d] [-s NAME] [-c DIR]\nflair attach -t NAME\nflair switch PATH|NAME\nflair ls\nflair kill-server\nflair test-headless --scenario PATH --output DIR [--timeout-ms 5000] [--config PATH] [--pixels WxH] [--scale S]"
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
            config,
            pixels,
            scale,
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
            match headless::run(
                scenario,
                &output,
                Duration::from_millis(timeout_ms),
                config.as_deref(),
                pixels,
                scale,
            ) {
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
            let path = path.unwrap_or_else(Config::default_path);
            let (config, error) = Config::load(&path);
            if let Some(error) = error {
                log::warn!("{error}; using defaults");
            }
            finish((|| {
                let socket = flair_server::socket::socket_path()?;
                let mut config = daemon::server_config(&config)?;
                config.path = path;
                flair_server::runtime::serve(config, &socket)
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
                let config_path = Config::default_path();
                let (config, _) = Config::load(&config_path);
                let link = daemon::connect(None, true)?;
                flair_gui::run_target(
                    config,
                    config_path,
                    link,
                    None,
                    Target::Name(session.name.clone()),
                    daemon::environment(),
                    Vec::new(),
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
            let config_path = path.clone().unwrap_or_else(Config::default_path);
            let (config, error) = Config::load(&config_path);
            if let Some(error) = error {
                log::warn!("{error}; using defaults");
            }
            let drive = match env::var_os("FLAIR_UI_SCRIPT") {
                Some(script) => match ui_script(Path::new(&script)) {
                    Ok(drive) => drive,
                    Err(error) => {
                        eprintln!("flair: {error}");
                        return ExitCode::from(2);
                    }
                },
                None => Vec::new(),
            };
            if env::var_os("FLAIR_STARTUP_PROBE").is_some() {
                eprintln!("probe config {:.1}", start.elapsed().as_secs_f64() * 1000.0);
            }
            let (client, worker) = if standalone {
                let (client, server) = channel_pair();
                let mut server_config = ServerConfig::from_config(&config);
                if let Some(path) = &path {
                    server_config.path.clone_from(path);
                }
                match spawn_in_process(server_config, server) {
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
                config_path,
                client,
                env::var_os("FLAIR_STARTUP_PROBE").map(|_| start),
                target,
                daemon::environment(),
                drive,
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

fn ui_script(path: &Path) -> Result<Vec<Drive>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut drive = Vec::new();
    for step in Scenario::parse(&text)?.steps {
        match step {
            Step::Key {
                key,
                ctrl,
                shift,
                alt,
            } => drive.push(Drive::Key(key_input(&key, ctrl, shift, alt)?)),
            Step::Text { text } => drive.push(Drive::Text(text)),
            Step::Sleep { ms } => drive.push(Drive::Sleep(Duration::from_millis(ms))),
            Step::Window { width, height } => drive.push(Drive::Resize { width, height }),
            Step::Wait { .. }
            | Step::Paste { .. }
            | Step::Resize { .. }
            | Step::Scroll { .. }
            | Step::Cursor { .. }
            | Step::Capture { .. } => {}
        }
    }
    Ok(drive)
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
