mod args;
mod ha;
mod tui;
mod types { pub mod lovelace; pub mod events; pub mod responses; pub mod app;}

use anyhow::Result;
use tokio::sync::mpsc;

use crate::args::Args;
use crate::ha::ha_worker;
use crate::tui::tui_worker;
use crate::types::app::AppEvent;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse()?;
    let (ui_tx, mut ui_rx) = mpsc::channel::<AppEvent>(256);
    let (ev_tx, mut ev_rx) = mpsc::channel::<AppEvent>(256);
    
    tokio::spawn(async move {
        if let Err(e) = ha_worker(&args.ha_url, &args.ha_token, &ui_tx, &mut ev_rx).await {
            // report error back to UI
            let full_msg = e
                .chain()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            let _ = ui_tx.send(AppEvent::Error(full_msg)).await;
        }
    });

    tui_worker(&mut ui_rx, &ev_tx).await
}
