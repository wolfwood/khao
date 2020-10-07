use reqwest;
use tokio;
use serde_json;

use std::fs::File;
use std::io::Write;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cache = std::path::Path::new("mycache.data");

    let json : serde_json::Value =
        if cache.exists() {
            let mut file = File::open(cache)?;

            serde_json::from_reader(file)?
        } else {
            let body = reqwest::get("https://api.mmoui.com/v4/game/ESO/filelist.json").await?;

            let mut file = File::create(cache)?;

            let body = body.text().await?;

            file.write_all(body.as_bytes())?;

            serde_json::from_str(&body)?
        };

    println!("{}", serde_json::to_string_pretty(&json)?);

    Ok(())
}
