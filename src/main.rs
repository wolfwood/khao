#![allow(dead_code)]

use reqwest;
use serde_json;
use tokio;

use futures_util::StreamExt;

use anyhow::Result;
use dirs::home_dir;
use glob::glob;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::io::BufReader;
use std::io::BufRead;
use std::collections::HashMap;
use regex::Regex;
use multimap::MultiMap;
use md5::{Md5,Digest};
use hex::FromHex;
use generic_array::GenericArray;

// URLs
const GLOBAL_CONFIG_URL: &str = "https://api.mmoui.com/v4/globalconfig.json";
const ESO_GAME_ID: &str = "ESO";
const GAME_CONFIG_URL: &str = "https://api.mmoui.com/v4/game/ESO/gameconfig.json";
const FILE_LIST_URL: &str = "https://api.mmoui.com/v4/game/ESO/filelist.json";
const FILE_DETAILS_URL: &str = "https://api.mmoui.com/v4/game/ESO/filedetails/";

// PATHs
macro_rules! _eso_proto_path {
    () => {
        "Elder Scrolls Online/live/AddOns/"
    };
}
macro_rules! _windows_path {
    () => {
        concat!("My Documents/", _eso_proto_path!())
    };
}
const WINDOWS_PATH: &str = _windows_path!();
const OSX_PATH: &str = concat!("Documents/", _eso_proto_path!());
const STEAM_PREFIX_PATH: &str = concat!(
    ".local/share/Steam/steamapps/compatdata/306130/pfx/drive_c/users/steamuser/",
    _windows_path!()
);

// API JSON Types
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    #[serde(alias = "gameID")]
    game_id: String,
    game_config: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalConfig {
    games: Vec<Game>,
}

#[derive(Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFeeds {
    file_list: String,
}

#[derive(Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameConfig {
    api_feeds: ApiFeeds,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileDetails {
    id: u16,
    title: String,
    version: String,
    file_name: String,
    download_uri: String,

    checksum: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddonInfo {
    path: String,
    add_on_version: String,
    optional_dependencies: Option<Vec<String>>,
    required_dependencies: Option<Vec<String>>,
    library: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    id: u16,
    title: String,
    version: String,
    library: Option<bool>,
    addons: Option<Vec<AddonInfo>>,

    checksum: String,

    // we parse out a version from the addons vec and stick it here
    #[serde(skip)]
    nested_version: Option<String>,
}

/*
/// User-defined configuration
#[derive(Serialize, Deserialize)]
struct Config {
    /// Optional plugin destination dir
    /// If not present, will be set to system default
    /// On OSX this is `~/Documents/Elder\ Scrolls\ Online/live/AddOns/`
    dest: Option<String>,
    /// List of add-on titles to manage, like `AUI - Advanced UI`.
    addons: Vec<String>,
}
 */

/// Information about a managed add-on
//#[derive(Serialize, Deserialize)]
struct InstalledAddon {
    /// The title of the addon (either matching that in `Config.addons` or a dep of one of those)
    title: String,
    /// Checksum to detect changes when `filelist.json` gets updated
    version: Option<String>,
    version_name: Option<String>,
    /// Path relative to `dest`
    name: String,
    path: String,
    is_lib: bool,
}

/*
/// Tool metadata about installed versions
#[derive(Serialize, Deserialize)]
struct Metadata {

    /// List of info about managed addons (or deps of those)
    installed_addons: Vec<InstalledAddon>,
}
*/

fn compare_versions (addon:&InstalledAddon, latest:&FileInfo) -> bool{
    match &addon.version {
        Some(v) => {
            if let Some(v2) = &latest.nested_version{
                if v == v2 {//println!("up to date: {}", &path);
                    return true;
                }
            }
        },
        None => ()
    }

    if let Some(v) = &addon.version_name {
        if v == &latest.version {//println!("up to date: {}", &path);
            return true;
        }
    }

    false
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

/*
fn read_config(path: &Path) -> Result<Config> {
    let file = File::open(path)?;

    println!("  let val = serde_json::from_reader(&file)?;
    Ok(val)
}*/

fn parse_addon_manifest(path: &Path) -> Result<Option<InstalledAddon>> {
    let mut is_lib = false;

    let file = File::open(path)?;
    let buffered = BufReader::new(file);

    let mut title = "".to_string();
    let mut version_name:Option<String> = None;
    let mut version:Option<String> = None;


    let filter_junk = Regex::new("\\|r|\\|[a-fA-F0-9]{7}|(?: )v?[0-9]{1,2}(?:\\.[0-9]{1,2}){1,3}").unwrap();

    let filter_noninstalled = Regex::new("Data File").unwrap();


    for l in buffered.lines() {
        if let Ok(line) = l {
            let mut iter = line.split_whitespace();
            if let Some("##") = iter.next() {
                match iter.next() {
                    Some("Title:") => {//println!("{:?}", line);
                        let foo = line.strip_prefix("## Title: ").unwrap().trim();

                        if filter_noninstalled.is_match(foo){ return Ok(None);}

                        title = filter_junk.replace_all(foo, "").trim().to_string();

                        if title.len() == 0 {println!("  parse of {} came up empty", line);}

                    },
                    Some("Version:") => {//println!("{:?}", line);
                                         //version_name = Some(iter.next().unwrap_or_default().to_string());
                                         version_name = match iter.next() {
                                             Some(vers) => Some(vers.to_string()),
                                             None => None,
                                         }
                    },
                    Some("AddOnVersion:") => {//println!("{:?}", line);
                                              version = match iter.next() {
                                                  Some(vers) => Some(vers.to_string()),
                                                  None => None,
                                              }
                    },
                    Some("IsLibrary:") => {//println!("{:?}", line);
                        is_lib = iter.next().unwrap().to_lowercase().parse()?
                    },
                    _ => {}
                }
            }
        }
    }


    Ok(Some(InstalledAddon{title: title,
                           version: version,
                           version_name: version_name,
                           path: path.parent().unwrap().to_str().unwrap().to_string(),
                           is_lib: is_lib,
                           name: path.parent().unwrap().file_name().unwrap().to_str().unwrap().to_string()}))
}

fn read_installed_addons() -> Result<HashMap<String,InstalledAddon>> {
    let addon_dir = home_dir().unwrap().join(if cfg!(macos) {
        OSX_PATH
    } else if cfg!(unix) {
        STEAM_PREFIX_PATH
    } else if cfg!(windows) {
        WINDOWS_PATH
    } else {
        "failzor"
    });

    println!("{}", addon_dir.as_path().to_str().unwrap());
    let mut val = HashMap::new();

    /*for entry in addon_dir.read_dir().expect("read_dir call failed") {
            if let Ok(entry) = entry {
                //println!("{:?}", entry.path());
                if entry.is_dir() {
                    for subentry in addon_dir.read_dir().expect("read_dir call failed") {
                        if let Ok(subentry) = subentry {
                        }
                    }
                }
            }
    }*/

    let pat = addon_dir.as_path().to_str().unwrap().to_owned() + "*/*.txt";
    let filter_spaces = Regex::new("\\s")?;

    for entry in glob(&pat).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                if let Some(addon) = parse_addon_manifest(&path)? {
                    val.insert(filter_spaces.replace_all(&addon.name, "").to_lowercase().to_string() , addon);
                }
            },
            Err(e) => println!("{:?}", e),
        }
    }

    Ok(val)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cache_dir = Path::new(".cache");
    let download_dir = cache_dir.join("downloads");
    let staging_dir = cache_dir.join("staging");
    // TODO add CLI configuration

    // TODO write to /tmp or something
    std::fs::create_dir_all(&cache_dir)?;
    std::fs::create_dir_all(&download_dir)?;

    // TODO We'll start by downloading plugins into this fake destination
    //std::fs::create_dir_all(".testdest")?;

    let file_list_path = cache_dir.join("filelist.json");
    // TODO verify there is a config
    // let config_path = Path::new("khao_config.json");
    // let metadata_path = Path::new("khao_metadata.json");

    let installed_map = read_installed_addons()?;

    let file_list = if file_list_path.exists() {
        read_file_list(&file_list_path)?
    } else {
        write_file_list(&file_list_path).await?
    };


    /*
     *    Unfortunately there is no global namespace for ESO addons.
     *  There are four candidates for how we identify addons, each with flaws:
     *     1) the "Title" field in the addon manifest:
     *        - color escapes are common in the title (but not in the api)
     *        - the API title doesn't come from or match the manifest, seems like maybe its supplied on upload
     *        - some authors put full version in the manifest title, this will never help us find an updated version
     *        - sometimes capitalization/spacing is different
     *    2) the directory name (path) of the addon
     *        - this is not unique, often there are patches and localizations that share the path
     *    3) the api ID
     *        - doesn't exist in the manifest, so we can only use this if we disregard existing installs and
     *              track separate metadata when installing to remember where it came from
     *    4) cache archives, compare checksums ( or keep api version metadata about installed addons)
     *       - doesn't accommodate install from other sources, existing installs
     */

    // for right now #2 gets most things right, just need some conflict resolution (maybe using author too)
    // and a map of string => vector of addons to handle multiple addons with the same path

    let filter_spaces = Regex::new("\\s")?;
    let mut file_map = MultiMap::new();
    for mut x in file_list {
        // XXX: why do I need to borrow?
        if let Some(addons) = &x.addons {
            for addon in addons {
                if None == addon.path.find('/') {
                    if let Some(ver) = &x.nested_version {
                        if &addon.add_on_version != ver {
                            println!("mismatch {} {}", &addon.add_on_version, &ver);
                        }
                    } else {
                       // println!("supp;ying missing version");
                        x.nested_version = Some(addon.add_on_version.clone());
                    }

                    file_map.insert(filter_spaces.replace_all(&addon.path, "").to_lowercase().to_string(), x);
                    break;
                }
            }
        }

        //file_map.insert(filter_spaces.replace_all(&x.title, "").to_lowercase().to_string(), &x);
    }

    //println!("{}", serde_json::to_string_pretty(&file_list)?);

    // TODO read config and download addons
    // let config = read_config(&config_path)?;
    // println!("{}", serde_json::to_string_pretty(&config)?);



    // check all installed items are up to date
    let mut outdated = HashMap::new();

    'outer: for (path, addon) in installed_map {
        if file_map.is_vec(&path) {
            for latest in file_map.get_vec(&path).unwrap() {
                //if addon.title == latest.title {
                    if compare_versions(&addon, &latest) {
                        continue 'outer;
                    }
                //}
            }
            println!("  multi-outdated: {}", &path);

        } else {
            let lookup = file_map.get(&path);

            if let Some(latest) = lookup {
                if compare_versions(&addon, &latest) {
                    continue;
                }
                println!("  outdated: {:?} {:?} {:?}", &path, &addon.version_name, &latest.version);
                outdated.insert(addon.path.clone(), latest);
            } else {
                println!("    {} {} no longer exists upstream!", addon.name, &path);
            }
        }
    }


    // check for a downloaded and installed version, and verify checksum

    // fetch, extract, install

    for (path, addon) in outdated {
        let addon_dest = Path::new(&path);
        let detail_url = FILE_DETAILS_URL.to_string() + &addon.id.to_string() + ".json";
        let array:Vec<FileDetails> = reqwest::get(&detail_url).await?.json().await?;
        let details = &array[0];

        let mut please_download = true;

        let addon_dir = download_dir.join(addon_dest.file_name().unwrap());
        std::fs::create_dir_all(&addon_dir)?;
        let filename = addon_dir.join(&details.file_name);
        // check for a downloaded archive and verify check sum, else
        if filename.exists() {
            let mut download_dest = File::open(&filename)?;

            let mut hasher = Md5::new();
            std::io::copy(&mut download_dest, &mut hasher)?;
            let hash = hasher.finalize();

            // XXX: clean this up?
            if hash == *GenericArray::from_slice(&<[u8; 16]>::from_hex(&details.checksum).unwrap()){
                please_download = false;
                println!("{:?} already downloaded", &download_dest);
            }
        }

        if please_download {
            let mut download_dest = File::create(&filename)?;
            let download_response = reqwest::get(&details.download_uri).await?;

            let mut content = download_response.bytes_stream();
            while let Some(item) = content.next().await {
                download_dest.write_all(&item?)?;
            }
        }
    }

    Ok(())
}
