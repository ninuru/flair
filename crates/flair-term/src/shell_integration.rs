use crate::pty::SpawnSpec;
use std::{
    ffi::OsString,
    fs, io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

pub(crate) struct ShellIntegration {
    path: PathBuf,
    shell_index: Option<usize>,
}

impl ShellIntegration {
    pub(crate) fn prepare(spec: &SpawnSpec) -> io::Result<Option<Self>> {
        let shell_index = (spec.program == "/usr/bin/env"
            && spec.args.first().is_some_and(|arg| arg == "-i"))
        .then(|| {
            spec.args
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, arg)| !arg.contains('='))
                .map(|(index, _)| index)
        })
        .flatten();
        let (program, args) = shell_index
            .map_or((spec.program.as_str(), spec.args.as_slice()), |index| {
                (spec.args[index].as_str(), &spec.args[index + 1..])
            });
        if Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())
            != Some("zsh")
            || !spec.env.iter().any(|(key, _)| key == "FLAIR_PROMPT")
            || args.iter().any(|arg| {
                arg == "--command"
                    || arg == "--no-rcs"
                    || (arg.starts_with('-')
                        && !arg.starts_with("--")
                        && arg[1..].contains(['c', 'f']))
            })
        {
            return Ok(None);
        }
        let mut template = std::env::temp_dir()
            .join("flair-zsh-XXXXXX")
            .as_os_str()
            .as_bytes()
            .to_vec();
        template.push(0);
        if unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) }.is_null() {
            return Err(io::Error::last_os_error());
        }
        template.pop();
        let integration = Self {
            path: PathBuf::from(OsString::from_vec(template)),
            shell_index,
        };
        fs::write(
            integration.path.join("flair.zsh"),
            include_str!("../../../contrib/flair.zsh"),
        )?;
        fs::write(integration.path.join(".zshenv"), ZSHENV)?;
        fs::write(integration.path.join(".zprofile"), ZPROFILE)?;
        fs::write(integration.path.join(".zshrc"), ZSHRC)?;
        Ok(Some(integration))
    }

    pub(crate) fn args(&self, spec: &SpawnSpec, env: &[(String, String)]) -> Vec<String> {
        match self.shell_index {
            Some(index) => spec.args[..index]
                .iter()
                .cloned()
                .chain(env.iter().map(|(key, value)| format!("{key}={value}")))
                .chain(spec.args[index..].iter().cloned())
                .collect(),
            None => spec.args.clone(),
        }
    }

    pub(crate) fn env(&self, spec: &SpawnSpec) -> io::Result<Vec<(String, String)>> {
        let path = self.path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "shell integration path is not UTF-8",
            )
        })?;
        let original = if spec.unset_env.iter().any(|key| key == "ZDOTDIR") {
            None
        } else {
            spec.env
                .iter()
                .rev()
                .find(|(key, _)| key == "ZDOTDIR")
                .map(|(_, value)| value.clone())
                .or_else(|| std::env::var("ZDOTDIR").ok())
        };
        Ok(vec![
            ("FLAIR_ZSH_DIR".into(), path.into()),
            (
                "FLAIR_ZSH_ZDOTDIR_SET".into(),
                if original.is_some() { "1" } else { "0" }.into(),
            ),
            ("FLAIR_ZSH_ZDOTDIR".into(), original.unwrap_or_default()),
            ("ZDOTDIR".into(), path.into()),
        ])
    }
}

impl Drop for ShellIntegration {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            log::warn!("Could not remove shell integration directory: {error}");
        }
    }
}

const ZSHENV: &str = r#"if [[ $FLAIR_ZSH_ZDOTDIR_SET == 1 ]]; then
    export ZDOTDIR=$FLAIR_ZSH_ZDOTDIR
else
    unset ZDOTDIR
fi
typeset -g _flair_integration_dir=$FLAIR_ZSH_DIR
unset FLAIR_ZSH_DIR FLAIR_ZSH_ZDOTDIR FLAIR_ZSH_ZDOTDIR_SET
[[ -o rcs && -r ${ZDOTDIR:-$HOME}/.zshenv ]] && source "${ZDOTDIR:-$HOME}/.zshenv"
typeset -g _flair_original_zdotdir_set=${+ZDOTDIR}
typeset -g _flair_original_zdotdir=${ZDOTDIR-}
export ZDOTDIR=$_flair_integration_dir
"#;
const ZPROFILE: &str = r#"if (( _flair_original_zdotdir_set )); then
    export ZDOTDIR=$_flair_original_zdotdir
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR:-$HOME}/.zprofile ]] && source "${ZDOTDIR:-$HOME}/.zprofile"
typeset -g _flair_original_zdotdir_set=${+ZDOTDIR}
typeset -g _flair_original_zdotdir=${ZDOTDIR-}
export ZDOTDIR=$_flair_integration_dir
"#;
const ZSHRC: &str = r#"if (( _flair_original_zdotdir_set )); then
    export ZDOTDIR=$_flair_original_zdotdir
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR:-$HOME}/.zshrc ]] && source "${ZDOTDIR:-$HOME}/.zshrc"
source "$_flair_integration_dir/flair.zsh"
unset _flair_integration_dir _flair_original_zdotdir_set _flair_original_zdotdir
"#;
