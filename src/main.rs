#![allow(dead_code)]

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

/// User-defined configuration
#[derive(Serialize, Deserialize)]
struct Config {
    /// Optional plugin destination dir
    /// If not present, will be set to system default
    /// On OSX this is `~/Documents/Elder\ Scrolls\ Online/live/AddOns/`
    dest: Option<String>,
    /// List of add-on titles to manage, like `AUI - Advanced UI`.
    addons: Vec<String>
}

/// Information about a managed add-on
#[derive(Serialize, Deserialize)]
struct InstalledAddon {
    /// The title of the addon (either matching that in `Config.addons` or a dep of one of those)
    title: String,
    /// Checksum to detect changes when `filelist.json` gets updated
    checksum: String,
    /// Path relative to `dest`
    path: String
}

/// Tool metadata about installed versions
#[derive(Serialize, Deserialize)]
struct Metadata {
    /// List of info about managed addons (or deps of those)
    installed_addons: Vec<InstalledAddon>
}

fn read_file_list(path: &Path) -> Result<Vec<FileInfo>> {
    let file = File::open(path)?;
    let val = serde_json::from_reader(&file)?;
    Ok(val)
}

async fn write_file_list(path: &Path) -> Result<Vec<FileInfo>> {
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

fn read_config(path: &Path) -> Result<Config> {
    let file = File::open(path)?;
    let val = serde_json::from_reader(&file)?;
    Ok(val)
}

#[tokio::main]
async fn main() -> Result<()> {
    // TODO add CLI configuration

    // TODO write to /tmp or something
    std::fs::create_dir_all(".cache")?;

    // TODO We'll start by downloading plugins into this fake destination
    std::fs::create_dir_all(".testdest")?;

    let file_list_path = Path::new(".cache/filelist.json");
    // TODO verify there is a config
    // let config_path = Path::new("khao_config.json");
    // let metadata_path = Path::new("khao_metadata.json");

    let file_list = if file_list_path.exists() {
        read_file_list(&file_list_path)?
    } else {
        write_file_list(&file_list_path).await?
    };

    println!("{}", serde_json::to_string_pretty(&file_list)?);

    // TODO read config and download addons
    // let config = read_config(&config_path)?;
    // println!("{}", serde_json::to_string_pretty(&config)?);

    Ok(())
}
