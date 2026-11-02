use anyhow::{Result, Context};
use std::{env, fs, path::{Path, PathBuf}};

// options and environment variables as in hass-cli
pub struct Args {
    pub server: String,
    pub token: String,
    pub dashboard: String,
}

impl Args {
    pub fn parse() -> Result<Self> {
        let mut options = Config::default();
        let mut config_file = None;
        let mut help = false;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let mut value = || args.next().with_context(|| format!("Missing value for {arg}"));
            match arg.as_str() {
                "-s" | "--server" => options.server = Some(value()?),
                "--token" => options.token = Some(value()?),
                "--dashboard" => options.dashboard = Some(value()?),
                "--config" => config_file = Some(PathBuf::from(value()?)),
                "-h" | "--help" => help = true,
                "-V" | "--version" => {
                    println!("hata {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                _ => anyhow::bail!("Unknown argument: {arg}\nSee `hata --help`."),
            }
        }

        // a file given in the options must exist, the usual one may not
        let required = config_file.is_some();
        let config_file = config_file.or_else(config_path);
        if help {
            print!("{}", usage(config_file.as_deref()));
            std::process::exit(0);
        }

        // the config file, overridden by environment variables, overridden by options
        let mut config = match &config_file {
            Some(path) if required || path.exists() => read_config(path)?,
            _ => Config::default(),
        };
        config.server = env::var("HASS_SERVER").ok().or(config.server);
        config.token = env::var("HASS_TOKEN").ok().or(config.token);
        config.dashboard = env::var("HASS_DASHBOARD").ok().or(config.dashboard);
        config.server = options.server.or(config.server);
        config.token = options.token.or(config.token);
        config.dashboard = options.dashboard.or(config.dashboard);

        Ok(Self {
            server: config.server.context("Missing server: --server, HASS_SERVER or `server` in the config file")?,
            token: config.token.context("Missing token: --token, HASS_TOKEN or `token` in the config file")?,
            dashboard: config.dashboard.unwrap_or_else(|| "dashboard-tui".to_string()),
        })
    }
}

fn usage(config_file: Option<&Path>) -> String {
    let config_file = match config_file {
        Some(path) if path.exists() => path.display().to_string(),
        Some(path) => format!("{} (not found)", path.display()),
        None => "none".to_string(),
    };
    format!(
"HATA - Home Assistant Terminal Access {version}
Shows a Home Assistant dashboard in the terminal.

Usage: hata [options]

Options:
  -s, --server <url>      Home Assistant URL, e.g. http://homeassistant.local:8123
      --token <token>     Long-lived access token, from your profile in Home Assistant
      --dashboard <name>  dashboard name from URL in HA [default: dashboard-tui]
      --config <file>     Config file to read instead of default one
  -V, --version           Print the version
  -h, --help              Print this help

Environment variables:
  HASS_SERVER, HASS_TOKEN, HASS_DASHBOARD

Config file:
  {config_file}
  `key = value` lines with the keys `server`, `token` and `dashboard`.

Options override environment variables, which override the config file.
",
        version = env!("CARGO_PKG_VERSION"),
    )
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

fn read_config(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
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
