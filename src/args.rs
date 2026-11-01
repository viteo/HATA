use anyhow::{Result, Context};
use std::{env, fs, io, path::PathBuf};

// options and environment variables as in hass-cli
pub struct Args {
    pub server: String,
    pub token: String,
    pub dashboard: String,
}

impl Args {
    pub fn parse() -> Result<Self> {
        // the config file, overridden by environment variables, overridden by options
        let mut config = read_config()?;
        config.server = env::var("HASS_SERVER").ok().or(config.server);
        config.token = env::var("HASS_TOKEN").ok().or(config.token);
        config.dashboard = env::var("HASS_DASHBOARD").ok().or(config.dashboard);

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args.next().with_context(|| format!("Missing value for {arg}"))?;
            match arg.as_str() {
                "--server" => config.server = Some(value),
                "--token" => config.token = Some(value),
                "--dashboard" => config.dashboard = Some(value),
                _ => anyhow::bail!("Unknown argument: {arg}"),
            }
        }

        Ok(Self {
            server: config.server.context("Missing server: --server, HASS_SERVER or `server` in the config file")?,
            token: config.token.context("Missing token: --token, HASS_TOKEN or `token` in the config file")?,
            dashboard: config.dashboard.unwrap_or_else(|| "dashboard-tui".to_string()),
        })
    }
}

/// The settings of the config file.
#[derive(Default)]
struct Config {
    server: Option<String>,
    token: Option<String>,
    dashboard: Option<String>,
}

/// `$XDG_CONFIG_HOME/hata/config`, or `~/.config/hata/config`, or `%APPDATA%\hata\config`
fn config_path() -> Option<PathBuf> {
    let config_folder = env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()).map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))?;
    Some(config_folder.join("hata").join("config"))
}

/// Nothing is set when there is no config file.
fn read_config() -> Result<Config> {
    let Some(path) = config_path() else { return Ok(Config::default()) };
    let text = match fs::read_to_string(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        text => text.with_context(|| format!("Failed to read {}", path.display()))?,
    };
    parse_config(&text).with_context(|| format!("Failed to read {}", path.display()))
}

/// `key = value` lines; empty lines and lines starting with `#` are skipped.
fn parse_config(text: &str) -> Result<Config> {
    let mut config = Config::default();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').with_context(|| format!("Line {}: expected `key = value`", number + 1))?;
        let value = Some(value.trim().to_string());
        match key.trim() {
            "server" => config.server = value,
            "token" => config.token = value,
            "dashboard" => config.dashboard = value,
            key => anyhow::bail!("Line {}: unknown key `{}`", number + 1, key),
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_has_keys_values_and_comments() {
        let config = parse_config("# my home\nserver = http://ha.local:8123\n\n  token=abc=def  \n").unwrap();
        assert_eq!(config.server.as_deref(), Some("http://ha.local:8123"));
        assert_eq!(config.token.as_deref(), Some("abc=def"));
        assert_eq!(config.dashboard, None);
    }

    #[test]
    fn config_with_a_typo_is_an_error() {
        assert!(parse_config("sever = http://ha.local:8123").is_err());
        assert!(parse_config("server http://ha.local:8123").is_err());
    }
}
