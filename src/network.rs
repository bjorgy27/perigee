use reqwest;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

const URL: &str = "https://www.space-track.org/basicspacedata/query/class/gp_history/NORAD_CAT_ID/25544/orderby/EPOCH desc/limit/1/format/xml ";
const LOGIN: &str = "https://www.space-track.org/ajaxauth/login";


fn write_file(file: &mut File, data: &str) -> std::io::Result<()> {
    file.write_all(data.as_bytes())?;
    Ok(())
}

pub fn get_sat_data() -> Result<(), Box<dyn std::error::Error>> {

    let client = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .build()?;

    let mut credentials = HashMap::new();
        credentials.insert("identity", "xxxxx");
        credentials.insert("password","xxxxxx");

    let response = client.post(LOGIN)
        .form(&credentials)
        .send()?;
    println!("login_status {}", response.status());

    let data = client.get(URL).send()?;
    let status = data.status();
    let body = data.text()?;

    println!("status = {}", status);
    //println!("body = {}", body);

    let mut file = File::create("ELSET.xml")?;
    write_file(&mut file, &body)?;

    Ok(())



} 