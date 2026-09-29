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
use nalgebra::{vector, SVector};
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
//Element sets side by side: 9 rows, one column per satellite (defined in the perigee-orbit crate)
pub use perigee_orbit::ElSetMatrix;



//Space-Track API
//Which orbits to pull is set by PERIGEE_ORBITS (.env or the environment):
//  leo  (default)   mean motion above 11.25 rev/day: the low passes Perigee was built for
//  geo              mean motion 0.99..1.01 rev/day, near-circular: the geostationary belt (GOES, ...)
//  all              both queries, merged
//A geostationary target never rises or sets, so it appears as a single pass that is always in
//progress: parked pointing rather than tracking, which is exactly what GOES HRIT wants.
const ST_LEO: &str = "MEAN_MOTION/>11.25";
const ST_GEO: &str = "MEAN_MOTION/0.99--1.01/ECCENTRICITY/<0.01";
const ST_URL: &str = "https://www.space-track.org/basicspacedata/query/class/gp/{f}/DECAY_DATE/null-val/OBJECT_TYPE/PAYLOAD/EPOCH/>now-30/orderby/NORAD_CAT_ID/format/json";

//(mode name, the element filters to fetch in order) from PERIGEE_ORBITS
pub fn orbit_mode() -> (String, Vec<&'static str>) {
    let mode = std::env::var("PERIGEE_ORBITS").unwrap_or_else(|_| "leo".into()).to_lowercase();
    match mode.trim() {
        "geo" | "gso" | "geostationary" => ("geo".into(), vec![ST_GEO]),
        "all" | "both" | "leo+geo" => ("all".into(), vec![ST_LEO, ST_GEO]),
        _ => ("leo".into(), vec![ST_LEO]),
    }
}
//const ST_URL: &str = "https://www.space-track.org/basicspacedata/query/class/gp_history/NORAD_CAT_ID/25544/orderby/EPOCH desc/limit/22/format/json";
const ST_LOGIN: &str = "https://www.space-track.org/ajaxauth/login";
//One small query that proves a session works: the newest element set of the ISS
const ST_PROBE: &str = "https://www.space-track.org/basicspacedata/query/class/gp/NORAD_CAT_ID/25544/format/json";




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



//Pulling element sets for the orbits PERIGEE_ORBITS asks for (LEO, the geostationary belt, or both)
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

    let (mode, filters) = orbit_mode();
    println!("Orbits = {mode}");

    //One query per filter, merged into a single array so ELSET.json keeps its shape
    let mut records: Vec<serde_json::Value> = Vec::new();
    for f in filters {
        let sat_data = client.get(ST_URL.replace("{f}", f)).send()?;
        let status = sat_data.status();
        let body = sat_data.text()?;
        println!("Space Track Status = {status}  ({f})");
        let part: Vec<serde_json::Value> = serde_json::from_str(&body)?;
        println!("  records = {}", part.len());
        records.extend(part);
    }

    let merged = serde_json::to_string(&records)?;
    let mut file = File::create("ELSET.json")?;
    write_file(&mut file, &merged)?;

    //The JSON body is an array of OMM objects -> Vec<Omm>
    let omms: Vec<Omm> = serde_json::from_str(&merged)?;

    Ok(omms)
} 



//"perigee login": log in with the credentials from the environment (or .env) and prove the session with
//the ISS probe query. One fact per printed line so a caller can follow along; Err (exit code 1) when the
//site refuses the credentials, answers with no records, or cannot be reached. Downloads nothing else.
pub fn login_check() -> Result<(), Error> {
    let identity = std::env::var("SPACETRACK_USER").map_err(|_| "SPACETRACK_USER is not set (put it in .env or the environment)")?;
    let password = std::env::var("SPACETRACK_PASS").map_err(|_| "SPACETRACK_PASS is not set (put it in .env or the environment)")?;
    println!("LOGIN identity {identity}");

    let client = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(25))
        .build()?;
    let mut credentials = HashMap::new();
        credentials.insert("identity", identity.as_str());
        credentials.insert("password", password.as_str());

    let started = std::time::Instant::now();
    let response = client.post(ST_LOGIN).form(&credentials).send()?;
    let status = response.status();
    let body = response.text()?;
    println!("LOGIN status {status} in {:.2} s", started.elapsed().as_secs_f64());
    //Space-Track answers 200 either way; a refused login carries {"Login":"Failed"} in the body
    if !status.is_success() || body.contains("Failed") {
        println!("LOGIN DENIED {}", body.trim());
        return Err("Space-Track refused the credentials".into());
    }

    let probe = client.get(ST_PROBE).send()?;
    let pstatus = probe.status();
    let ptext = probe.text()?;
    let records: Vec<Omm> = serde_json::from_str(&ptext).unwrap_or_default();
    match records.first() {
        Some(o) => println!("PROBE status {pstatus}  ISS {}  epoch {}  {:.4} rev/day", o.norad_cat_id, o.epoch, o.mean_motion),
        None => {
            println!("PROBE status {pstatus}  no records: {}", ptext.chars().take(120).collect::<String>().trim());
            return Err("the session did not return data".into());
        }
    }
    println!("LOGIN OK");
    Ok(())
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