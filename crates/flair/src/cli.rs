use lexopt::{Arg, ValueExt};
use std::{ffi::OsString, path::PathBuf};

pub enum Args {
    Help,
    Version,
    Gui {
        config: Option<PathBuf>,
    },
    Headless {
        scenario: PathBuf,
        output: PathBuf,
        timeout_ms: u64,
    },
}
impl Args {
    pub fn parse_from(
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<Self, lexopt::Error> {
        let mut parser = lexopt::Parser::from_args(args);
        let mut config = None;
        let mut scenario = None;
        let mut output = None;
        let mut timeout_ms = 5000;
        let mut headless = false;
        while let Some(arg) = parser.next()? {
            match arg {
                Arg::Long("help") | Arg::Short('h') => return Ok(Self::Help),
                Arg::Long("version") | Arg::Short('V') => return Ok(Self::Version),
                Arg::Long("config") => config = Some(PathBuf::from(parser.value()?)),
                Arg::Long("standalone") => {}
                Arg::Value(value) if value == "test-headless" && !headless => headless = true,
                Arg::Long("scenario") if headless => {
                    scenario = Some(PathBuf::from(parser.value()?))
                }
                Arg::Long("output") if headless => output = Some(PathBuf::from(parser.value()?)),
                Arg::Long("timeout-ms") if headless => timeout_ms = parser.value()?.parse()?,
                _ => return Err(arg.unexpected()),
            }
        }
        if headless {
            if config.is_some() {
                return Err("headless tests use scenario config, not --config".into());
            }
            if timeout_ms == 0 || timeout_ms > 300_000 {
                return Err("--timeout-ms must be between 1 and 300000".into());
            }
            Ok(Self::Headless {
                scenario: scenario.ok_or("--scenario is required")?,
                output: output.ok_or("--output is required")?,
                timeout_ms,
            })
        } else {
            Ok(Self::Gui { config })
        }
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
