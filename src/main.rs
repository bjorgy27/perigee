/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Perigee-
/// Autonomous satellite tracking: catalog-wide pass prediction, transmitter aware target ranking, 
/// and closed loop dish pointing with telemetry capture.
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
mod satnogs;
    use satnogs::get_norad_id;

mod spacetrack;
    use spacetrack::get_sat_data;


fn main() ->  Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    //Logging into space-track and requesting 
    //get_sat_data()?;

    get_norad_id()?;

    //Parse TLE
    /* 
    let xml = std::fs::read_to_string("ELSET.xml")?;
    let elements = parse_mean_elements(&xml)?;
    println!("Matrix = {:?}", &elements);

    */
    
    Ok(())

} 

   

