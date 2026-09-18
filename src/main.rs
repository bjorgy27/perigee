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
    use satnogs::norad_ids;

mod spacetrack;
    use spacetrack::get_sat_data;
    use spacetrack::parse_mean_elements;

mod catalog;
    use catalog::intersect;


fn main() ->  Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    //Logging into space-track and requesting data
    let elset = get_sat_data()?;
    println!("Space-Track records: {}", elset.len());

    let transmitters = get_norad_id()?;
    let norads = norad_ids(&transmitters);

    //Satellite Elements in 9xN matrix form
    let elements = parse_mean_elements(&elset)?;
    

    //Satellite Elements in 9xN matrix form that that contain only desired NORADs
    let sorted_sats = intersect(&elements, &norads);
    println!("Useable Sats = {}", sorted_sats.ncols());

    std::fs::write("SORTED_SATS.json", serde_json::to_string(&sorted_sats)?)?;

    Ok(())

} 

   

