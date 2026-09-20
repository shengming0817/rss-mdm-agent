//! Manual credential-source acceptance. Uses the production registry and credential broker.
//! The chosen source is read by the Host; secrets never go to stdout or an evidence file.
use rss_mdm_desktop::composition::{credentials, users::Users};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("private acceptance directory required")?,
    );
    let mut users = Users::open(&root)?;
    users.select("Connection acceptance")?;
    let stop = CancellationToken::new();
    credentials::serve(
        root.join("credentials.sock"),
        Arc::new(Mutex::new(users)),
        stop.clone(),
    )
    .await?;
    use tokio::io::AsyncReadExt;
    let mut input = [0_u8; 1];
    let _ = tokio::io::stdin().read(&mut input).await?;
    stop.cancel();
    Ok(())
}
