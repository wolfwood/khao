//extern crate serde_json;
extern crate reqwest;

//use serde_json;
use reqwest;

fn main() {
    let body = reqwest::get("https://api.mmoui.com/v3/game/ESO/filelist.json")?
        .json()?;


}
