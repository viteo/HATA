mod app;
mod args;
mod ha;
mod tui;
mod types { pub mod lovelace; pub mod events; pub mod responses;}

use anyhow::Result;
use tokio::sync::mpsc;

use crate::app::{AppEvent};
use crate::args::Args;
use crate::ha::ha_worker;
use crate::tui::tui_worker;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse()?;
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
    tokio::spawn(async move {
        if let Err(e) = ha_worker(&args.ha_url, &args.ha_token, &tx).await {
            // report error back to UI
            let full_msg = e
                .chain()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            let _ = tx.send(AppEvent::Error(full_msg)).await;
        }
    });

    tui_worker(&mut rx).await
}
