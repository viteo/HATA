use anyhow::{Result, Context};
pub struct Args {
    pub ha_url: String,
    pub ha_token: String,
}

impl Args {
    pub fn parse() -> Result<Self> {
        let mut ha_url = None;
        let mut ha_token = None;

        //check passed arguments
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--ha-url" => {
                    ha_url = Some(args.next().context("Missing value for --ha-url")?);
                }
                "--ha-token" => {
                    ha_token = Some(args.next().context("Missing value for --ha-token")?);
                }
                _ => return Err(anyhow::anyhow!("Unknown argument: {}", arg)),
            }
        }

        //or check the env
        let ha_url = ha_url.or_else(|| std::env::var("HA_URL").ok());
        let ha_token = ha_token.or_else(|| std::env::var("HA_TOKEN").ok());

        //collect
        Ok(Self {
            ha_url: ha_url.context("Missing --ha-url or HA_URL environment variable")?,
            ha_token: ha_token.context("Missing --ha-token or HA_TOKEN environment variable")?,
        })
    }
}