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
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;



//SatNOGS DB Api
const SN_URL: &str = "https://db.satnogs.org/api/transmitters/";



//Pulling list of every transmitting satellite + transmit frequency
pub fn get_norad_id() -> Result<(), Box<dyn std::error::Error>>{

    let norad_ids = reqwest::blocking::get(SN_URL)?;
    let status = norad_ids.status();
    let body = norad_ids.text()?;

    println!("SatNOGS Status = {}", status);

    let mut file = File::create("NORADs.xml")?;
    write_file(&mut file, &body)?;

    Ok(())

}




fn write_file(file: &mut File, data: &str) -> std::io::Result<()> {
    file.write_all(data.as_bytes())?;
    Ok(())
}