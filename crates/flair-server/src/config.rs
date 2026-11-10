use flair_core::{
    config::{Config, PromptConfig},
    keys::KeysConfig,
};
use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub scrollback: usize,
    pub keys: KeysConfig,
    pub prompt: PromptConfig,
    pub path: PathBuf,
}

impl ServerConfig {
    pub fn from_config(config: &Config) -> ServerConfig {
        let mut env: Vec<_> = config
            .env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        env.push(("TERM_PROGRAM".into(), "flair".into()));
        env.push(("FLAIR_PROMPT".into(), config.prompt.style.name().into()));
        env.push((
            "FLAIR_PROMPT_SEPARATOR".into(),
            config.prompt.separator.name().into(),
        ));
        ServerConfig {
            program: config.shell.program.clone(),
            args: config.shell.args.clone(),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env,
            scrollback: config.scrollback.lines,
            keys: config.keys.clone(),
            prompt: config.prompt.clone(),
            path: Config::default_path(),
        }
    }

    pub fn reloaded(&self, config: &Config) -> ServerConfig {
        let mut next = ServerConfig::from_config(config);
        next.cwd = self.cwd.clone();
        next.path = self.path.clone();
        next.env
            .extend(self.env.iter().filter(|(key, _)| key == "FLAIR").cloned());
        next
    }
}
