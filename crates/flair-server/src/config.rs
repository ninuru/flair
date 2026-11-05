use flair_core::config::Config;
use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub scrollback: usize,
}

impl ServerConfig {
    pub fn from_config(config: &Config) -> ServerConfig {
        let mut env: Vec<_> = config
            .env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        env.push(("TERM_PROGRAM".into(), "flair".into()));
        ServerConfig {
            program: config.shell.program.clone(),
            args: config.shell.args.clone(),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env,
            scrollback: config.scrollback.lines,
        }
    }
}
