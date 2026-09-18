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
use roxmltree::{Document, Node};
use nalgebra::{vector, Vector6, Matrix6xX};
type Error = Box<dyn std::error::Error>;



//Space-Track API
const ST_URL: &str = "https://www.space-track.org/basicspacedata/query/class/gp_history/NORAD_CAT_ID/25544/orderby/EPOCH desc/limit/2/format/xml";
const ST_LOGIN: &str = "https://www.space-track.org/ajaxauth/login";



//Pulling TLEs for every LEO satellite 
pub fn get_sat_data() -> Result<(), Box<dyn std::error::Error>> {

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

    let mut file = File::create("ELSET.xml")?;
    write_file(&mut file, &body)?;

    Ok(())
} 

/* 
//Parsing one node at a time
fn parse_one(mean_elements_node: Node) -> Result<Vector6<f64>, Error> {
    let get = |tag: &str| -> Result<String, Error> {
        mean_elements_node
            .children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(str::to_string)
            .ok_or_else(|| format!("{tag} not found").into())
    };

    Ok(vector![
        get("MEAN_MOTION")?.parse()?,
        get("ECCENTRICITY")?.parse()?,
        get("INCLINATION")?.parse()?,
        get("RA_OF_ASC_NODE")?.parse()?,
        get("ARG_OF_PERICENTER")?.parse()?,
        get("MEAN_ANOMALY")?.parse()?,
    ])
}


pub fn parse_mean_elements(xml: &str) -> Result<Matrix6xX<f64>, Error> {
    let doc = Document::parse(xml)?;

    let columns: Vec<Vector6<f64>> = doc
        .descendants()
        .filter(|n| n.has_tag_name("meanElements"))
        .map(parse_one)
        .collect::<Result<_, _>>()?;

    if columns.is_empty() {
        return Err("no meanElements nodes found".into());
    }
  
    Ok(Matrix6xX::from_columns(&columns))
    
}

*/

fn write_file(file: &mut File, data: &str) -> std::io::Result<()> {
    file.write_all(data.as_bytes())?;
    Ok(())
}
