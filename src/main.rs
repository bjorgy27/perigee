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

mod orbitprop;
    use orbitprop::sv_from_coe;
mod rk4;
    use rk4::propagate;
mod constants;

use nalgebra::{Vector6, Matrix6xX};
use spacetrack::ElSetMatrix;


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
    let sorted_sats  = intersect(&elements, &norads);
    println!("Useable Sats = {}", sorted_sats.ncols());
    std::fs::write("SORTED_SATS.json", serde_json::to_string(&sorted_sats)?)?;

    //Calculate initial state vectors + epoch + sim dt all in julian time 
    let (x0, jd_epoch, t_end, dt) = sv_from_coe(&sorted_sats);
    //println!("State Vector = {:?}", x0);

    //Propagate every satellite to t_end, 60 s step
    let orbits = propagate_all(&x0, &dt, &sorted_sats, 60.0);
    println!("Propagated {} orbits, {} columns each", orbits.len(), orbits[0].ncols());
    std::fs::write("ORBIT_DATA.json", serde_json::to_string(&orbits)?)?;

    Ok(())

} 


//Propagate every column of x0 from its own epoch to t_end (dt[col] seconds), step h.
//Returns one 6 x M trajectory matrix per satellite, in the same column order as sorted_sats.
fn propagate_all(x0: &Matrix6xX<f64>, dt: &[f64], sorted_sats: &ElSetMatrix, h: f64) -> Vec<Matrix6xX<f64>> {

    let mut orbits: Vec<Matrix6xX<f64>> = Vec::new();

    for col in 0..x0.ncols() {
        let y0: Vector6<f64> = x0.column(col).into();
        let bstar = sorted_sats[(8, col)];
        orbits.push(propagate(y0, dt[col], h, bstar));
    }

    orbits
}

   

