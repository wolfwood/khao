use reqwest;
use tokio;
use serde_json;

use std::fs::File;

// Traits
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
            let root_json : serde_json::Value = reqwest::get("https://api.mmoui.com/v4/globalconfig.json").await?.json().await?;

            let gameconfig_url = &*root_json.get("games").unwrap().get(1).unwrap().get("gameConfig").unwrap();

            if gameconfig_url.as_str().unwrap() != "https://api.mmoui.com/v4/game/ESO/gameconfig.json" {
                eprintln!("game config url changed: {}", gameconfig_url);
            }

            let gameconfig_json : serde_json::Value = reqwest::get(gameconfig_url.as_str().unwrap()).await?.json().await?;


            let filelist_url = &*gameconfig_json.get("apiFeeds").unwrap().get("fileList").unwrap();

            if filelist_url.as_str().unwrap() != "https://api.mmoui.com/v4/game/ESO/filelist.json" {
                println!("filelist url changed: {}", filelist_url);
            }


            let body = reqwest::get(filelist_url.as_str().unwrap()).await?;

            let mut file = File::create(cache)?;

            let body = body.text().await?;

            file.write_all(body.as_bytes())?;

            serde_json::from_str(&body)?
        };

    println!("{}", serde_json::to_string_pretty(&json)?);

    Ok(())
}
