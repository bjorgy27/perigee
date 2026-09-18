/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// SatNOGS DB API + Parser
/// Download updated list of transmitting satellites + transmit frequencies on runtime
/// 
/// /////////////////////////////////////////////////////////////////////////////////////////////////////
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
use reqwest;
use std::fs::File;
use std::io::Write;
use serde::Deserialize;
type Error = Box<dyn std::error::Error>;



//SatNOGS DB Api
const SN_URL: &str = "https://db.satnogs.org/api/transmitters/?alive=true&status=active&type=Transmitter&format=json";



#[derive(Deserialize, Debug, Clone)]
pub struct Transmitter {
    pub norad_cat_id: Option<u32>,
    pub description: String,
    pub downlink_low: Option<u64>,
    pub downlink_high: Option<u64>,
    pub mode: Option<String>,
    pub baud: Option<f64>,
    pub service: String,
}



//Pulling list of every transmitting satellite + transmit frequency
pub fn get_norad_id() -> Result<Vec<Transmitter>, Error> {

    let norad_ids = reqwest::blocking::get(SN_URL)?;
    let status = norad_ids.status();
    let body = norad_ids.text()?;

    println!("SatNOGS Status = {}", status);

    let mut file = File::create("NORADs.json")?;
    write_file(&mut file, &body)?;

    //The JSON body is an array of transmitter objects -> Vec<Transmitter>
    let transmitters: Vec<Transmitter> = serde_json::from_str(&body)?;

    Ok(transmitters)

}



fn write_file(file: &mut File, data: &str) -> std::io::Result<()> {
    file.write_all(data.as_bytes())?;
    Ok(())
}

pub fn norad_ids(transmitters: &[Transmitter]) -> Vec<u32> {
    transmitters.iter().filter_map(|t| t.norad_cat_id).collect()
}
