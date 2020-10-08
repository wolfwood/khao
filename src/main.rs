use reqwest;
use serde_json;
use tokio;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

const GLOBAL_CONFIG_URL: &str = "https://api.mmoui.com/v4/globalconfig.json";
const ESO_GAME_ID: &str = "ESO";
const GAME_CONFIG_URL: &str = "https://api.mmoui.com/v4/game/ESO/gameconfig.json";
const FILE_LIST_URL: &str = "https://api.mmoui.com/v4/game/ESO/filelist.json";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    #[serde(alias = "gameID")]
    game_id: String,
    game_config: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalConfig {
    games: Vec<Game>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFeeds {
    file_list: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameConfig {
    api_feeds: ApiFeeds,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {}

fn read_cache(path: &Path) -> Result<Vec<FileInfo>> {
    let file = File::open(path)?;
    let val = serde_json::from_reader(&file)?;
    Ok(val)
}

async fn write_cache(path: &Path) -> Result<Vec<FileInfo>> {
    let global_config: GlobalConfig = reqwest::get(GLOBAL_CONFIG_URL).await?.json().await?;

    let game = global_config
        .games
        .iter()
        .find(|g| g.game_id == ESO_GAME_ID)
        .expect("No ESO def found");
    let game_config_url = &game.game_config;

    if game_config_url != GAME_CONFIG_URL {
        eprintln!("Game config url changed: {}", game.game_config);
    }

    let game_config: GameConfig = reqwest::get(game_config_url).await?.json().await?;

    eprintln!(
        "Game config: {}",
        serde_json::to_string_pretty(&game_config)?
    );

    let file_list_url = game_config.api_feeds.file_list;

    if file_list_url != FILE_LIST_URL {
        eprintln!("Filelist url changed: {}", file_list_url);
    }

    let res = reqwest::get(&file_list_url).await?;

    let body = res.text().await?;

    let file_list: Vec<FileInfo> = serde_json::from_str(&body)?;

    let mut file = File::create(path)?;

    file.write_all(body.as_bytes())?;

    Ok(file_list)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cache = Path::new("mycache.data");

    let file_list = if cache.exists() {
        read_cache(&cache)?
    } else {
        write_cache(&cache).await?
    };

    println!("{}", serde_json::to_string_pretty(&file_list)?);

    Ok(())
}
