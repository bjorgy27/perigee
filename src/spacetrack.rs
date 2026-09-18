/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Space-Track API + Parser
/// Login into Space-Track and download TLEs for all LEO satellites
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
use std::fmt::Display;
use std::str::FromStr;
use serde::{Deserialize, Deserializer};
use nalgebra::{vector, SVector, OMatrix, Const, Dyn};
type Error = Box<dyn std::error::Error>;

//One element set, 9 rows:
//  0 NORAD ID
//  1 epoch as Julian date
//  2 mean motion        (rev/day)
//  3 eccentricity
//  4 inclination        (deg)
//  5 RA of asc node     (deg)
//  6 arg of pericenter  (deg)
//  7 mean anomaly       (deg)
//  8 B*                 (1/earth radii)
pub type ElSet = SVector<f64, 9>;
//Element sets side by side: 9 rows, one column per satellite
pub type ElSetMatrix = OMatrix<f64, Const<9>, Dyn>;



//Space-Track API
const ST_URL: &str = "https://www.space-track.org/basicspacedata/query/class/gp/MEAN_MOTION/>11.25/DECAY_DATE/null-val/OBJECT_TYPE/PAYLOAD/orderby/NORAD_CAT_ID/format/json";
const ST_LOGIN: &str = "https://www.space-track.org/ajaxauth/login";




#[derive(Deserialize, Debug, Clone)]
pub struct Omm {
    #[serde(rename = "NORAD_CAT_ID", deserialize_with = "number_from_str")]
    pub norad_cat_id: u32,
    #[serde(rename = "EPOCH")]
    pub epoch: String,
    #[serde(rename = "MEAN_MOTION", deserialize_with = "number_from_str")]
    pub mean_motion: f64,
    #[serde(rename = "ECCENTRICITY", deserialize_with = "number_from_str")]
    pub eccentricity: f64,
    #[serde(rename = "INCLINATION", deserialize_with = "number_from_str")]
    pub inclination: f64,
    #[serde(rename = "RA_OF_ASC_NODE", deserialize_with = "number_from_str")]
    pub ra_of_asc_node: f64,
    #[serde(rename = "ARG_OF_PERICENTER", deserialize_with = "number_from_str")]
    pub arg_of_pericenter: f64,
    #[serde(rename = "MEAN_ANOMALY", deserialize_with = "number_from_str")]
    pub mean_anomaly: f64,
    #[serde(rename = "BSTAR", deserialize_with = "number_from_str")]
    pub bstar: f64,
}

//Reads a JSON string and parses it into whatever number type the field is
fn number_from_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}



//Pulling TLEs for every LEO satellite 
pub fn get_sat_data() -> Result<Vec<Omm>, Error> {

    let client = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .build()?;

    let identity = std::env::var("SPACETRACK_USER")?;
    let password = std::env::var("SPACETRACK_PASS")?;

    let mut credentials = HashMap::new();
        credentials.insert("identity", identity.as_str());
        credentials.insert("password", password.as_str());

    let response = client.post(ST_LOGIN)
        .form(&credentials)
        .send()?;
    println!("login_status {}", response.status());

    let sat_data = client.get(ST_URL).send()?;
    let status = sat_data.status();
    let body = sat_data.text()?;

    println!("Space Track Status = {}", status);
    //println!("body = {}", body);

    let mut file = File::create("ELSET.json")?;
    write_file(&mut file, &body)?;

    //The JSON body is an array of OMM objects -> Vec<Omm>
    let omms: Vec<Omm> = serde_json::from_str(&body)?;

    Ok(omms)
} 



//One OMM record -> one 9-row column
fn to_elset(omm: &Omm) -> Result<ElSet, Error> {
    Ok(vector![
        omm.norad_cat_id as f64,
        epoch_to_jd(&omm.epoch)?,
        omm.mean_motion,
        omm.eccentricity,
        omm.inclination,
        omm.ra_of_asc_node,
        omm.arg_of_pericenter,
        omm.mean_anomaly,
        omm.bstar,
    ])
}

//Stack every satellite's element set into a 9 x N matrix, one column per satellite
pub fn parse_mean_elements(omms: &[Omm]) -> Result<ElSetMatrix, Error> {

    let columns: Vec<ElSet> = omms
        .iter()
        .map(to_elset)
        .collect::<Result<_, _>>()?;

    if columns.is_empty() {
        return Err("no OMM records found".into());
    }
  
    Ok(ElSetMatrix::from_columns(&columns))
    
}



fn write_file(file: &mut File, data: &str) -> std::io::Result<()> {
    file.write_all(data.as_bytes())?;
    Ok(())
}


//Space-Track epoch string -> Julian date
fn epoch_to_jd(epoch: &str) -> Result<f64, Error> {
    let dt = chrono::NaiveDateTime::parse_from_str(epoch, "%Y-%m-%dT%H:%M:%S%.f")?;
    let unix_seconds = dt.and_utc().timestamp() as f64
        + dt.and_utc().timestamp_subsec_micros() as f64 * 1e-6;
    //Unix epoch (1970-01-01T00:00:00) is JD 2440587.5
    Ok(unix_seconds / 86400.0 + 2440587.5)
}