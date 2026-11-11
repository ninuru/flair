use flair_core::proto::Target;
use lexopt::{Arg, ValueExt};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq)]
pub enum Args {
    Help,
    Version,
    Gui {
        config: Option<PathBuf>,
        target: Target,
        standalone: bool,
    },
    Server {
        config: Option<PathBuf>,
    },
    List,
    KillServer,
    Switch(Target),
    Open(PathBuf),
    Create {
        config: Option<PathBuf>,
        target: Target,
    },
    Headless {
        scenario: PathBuf,
        output: PathBuf,
        timeout_ms: u64,
        config: Option<PathBuf>,
        pixels: Option<(u32, u32)>,
        scale: f64,
    },
}

impl Args {
    pub fn parse_from(
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<Self, lexopt::Error> {
        let mut parser = lexopt::Parser::from_args(args);
        let mut command = None;
        let mut config = None;
        let mut scenario = None;
        let mut output = None;
        let mut timeout_ms = 5000;
        let mut pixels = None;
        let mut scale: f64 = 1.0;
        let mut standalone = false;
        let mut detached = false;
        let mut name = None;
        let mut root = None;
        let mut target = None;
        let mut file = None;
        while let Some(arg) = parser.next()? {
            match arg {
                Arg::Long("help") | Arg::Short('h') => return Ok(Self::Help),
                Arg::Long("version") | Arg::Short('V') => return Ok(Self::Version),
                Arg::Long("config") => config = Some(PathBuf::from(parser.value()?)),
                Arg::Long("standalone") => standalone = true,
                Arg::Short('d') if command.as_deref() == Some("new") => detached = true,
                Arg::Short('s') if command.as_deref() == Some("new") => {
                    name = Some(parser.value()?.string()?)
                }
                Arg::Short('c') if command.as_deref() == Some("new") => {
                    root = Some(PathBuf::from(parser.value()?))
                }
                Arg::Short('t') if command.as_deref() == Some("attach") => {
                    target = Some(Target::Name(parser.value()?.string()?))
                }
                Arg::Long("scenario") if command.as_deref() == Some("test-headless") => {
                    scenario = Some(PathBuf::from(parser.value()?))
                }
                Arg::Long("output") if command.as_deref() == Some("test-headless") => {
                    output = Some(PathBuf::from(parser.value()?))
                }
                Arg::Long("timeout-ms") if command.as_deref() == Some("test-headless") => {
                    timeout_ms = parser.value()?.parse()?
                }
                Arg::Long("pixels") if command.as_deref() == Some("test-headless") => {
                    pixels = Some(parse_pixels(&parser.value()?.string()?)?)
                }
                Arg::Long("scale") if command.as_deref() == Some("test-headless") => {
                    scale = parser.value()?.parse()?
                }
                Arg::Value(value) if command.is_none() => {
                    let value = value.string()?;
                    if ![
                        "server",
                        "new",
                        "attach",
                        "switch",
                        "open",
                        "ls",
                        "kill-server",
                        "test-headless",
                    ]
                    .contains(&value.as_str())
                    {
                        return Err(format!("unknown command: {value}").into());
                    }
                    command = Some(value);
                }
                Arg::Value(value) if command.as_deref() == Some("open") && file.is_none() => {
                    file = Some(
                        std::path::absolute(PathBuf::from(value))
                            .map_err(|e| lexopt::Error::from(e.to_string()))?,
                    );
                }
                Arg::Value(value) if command.as_deref() == Some("switch") && target.is_none() => {
                    let path = PathBuf::from(&value);
                    target = Some(
                        if path.components().count() > 1
                            || path.is_absolute()
                            || value == "."
                            || value == ".."
                        {
                            Target::Path(
                                std::fs::canonicalize(&path)
                                    .map_err(|e| format!("{}: {e}", path.display()))?,
                            )
                        } else {
                            Target::Name(value.string()?)
                        },
                    );
                }
                _ => return Err(arg.unexpected()),
            }
        }
        let cwd = || std::env::current_dir().map_err(|e| lexopt::Error::from(e.to_string()));
        if standalone && command.is_some() {
            return Err("--standalone cannot be combined with a command".into());
        }
        match command.as_deref() {
            None => Ok(Self::Gui {
                config,
                target: Target::Main,
                standalone,
            }),
            Some("server") => Ok(Self::Server { config }),
            Some("new") => {
                let root = std::fs::canonicalize(root.unwrap_or(cwd()?))
                    .map_err(|e| lexopt::Error::from(e.to_string()))?;
                let target = Target::Create { name, root };
                if detached {
                    Ok(Self::Create { config, target })
                } else {
                    Ok(Self::Gui {
                        config,
                        target,
                        standalone: false,
                    })
                }
            }
            Some("attach") => Ok(Self::Gui {
                config,
                target: target.ok_or("attach requires -t NAME")?,
                standalone: false,
            }),
            Some("switch") => {
                if config.is_some() {
                    return Err("switch does not accept --config".into());
                }
                Ok(Self::Switch(target.ok_or("switch requires PATH or NAME")?))
            }
            Some("open") => {
                if config.is_some() {
                    return Err("open does not accept --config".into());
                }
                Ok(Self::Open(file.ok_or("open requires FILE")?))
            }
            Some("ls") | Some("kill-server") => {
                if config.is_some() {
                    return Err("this command does not accept --config".into());
                }
                if command.as_deref() == Some("ls") {
                    Ok(Self::List)
                } else {
                    Ok(Self::KillServer)
                }
            }
            Some("test-headless") => {
                if !(scale.is_finite() && (0.25..=8.0).contains(&scale)) {
                    return Err("--scale must be between 0.25 and 8".into());
                }
                if timeout_ms == 0 || timeout_ms > 300_000 {
                    return Err("--timeout-ms must be between 1 and 300000".into());
                }
                Ok(Self::Headless {
                    scenario: scenario.ok_or("--scenario is required")?,
                    output: output.ok_or("--output is required")?,
                    timeout_ms,
                    config,
                    pixels,
                    scale,
                })
            }
            Some(value) => Err(format!("unknown command: {value}").into()),
        }
    }
}

fn parse_pixels(value: &str) -> Result<(u32, u32), lexopt::Error> {
    let invalid = || lexopt::Error::from(format!("--pixels expects WxH, got {value:?}"));
    let (width, height) = value.split_once('x').ok_or_else(invalid)?;
    let width: u32 = width.parse().map_err(|_| invalid())?;
    let height: u32 = height.parse().map_err(|_| invalid())?;
    if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
        return Err("--pixels must be between 1x1 and 16384x16384".into());
    }
    Ok((width, height))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
