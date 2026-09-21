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

//Orbit math lives in the perigee-orbit crate (./orbit) so the TV viewer can run the same integrator
    use perigee_orbit::{sv_from_coe, propagate_all};
mod geodesy;
mod categories;
mod passes;
mod score;

use nalgebra::Matrix6xX;
use spacetrack::ElSetMatrix;


fn main() ->  Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    //"perigee rank": re-score from the saved files, no Space-Track / SatNOGS fetch, no propagation
    if std::env::args().nth(1).as_deref() == Some("rank") {
        return rank_only();
    }
    //"perigee categories": rebuild CATEGORIES.json from the saved files + CelesTrak groups, nothing else
    if std::env::args().nth(1).as_deref() == Some("categories") {
        return categories_only();
    }
    
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

    //Satellite types for the viewer's TYPE filter (CelesTrak groups + SatNOGS services + name rules)
    let tracked: Vec<u32> = (0..sorted_sats.ncols()).map(|c| sorted_sats[(0, c)] as u32).collect();
    match categories::build(&tracked, &transmitters, true).and_then(|c| categories::write(&c)) {
        Ok(()) => {}
        Err(e) => println!("WARNING: categories not written: {e}"),
    }

    //Calculate initial state vectors + epoch + sim dt all in julian time; propagate 12 h past now
    let t_end = geodesy::now_jd() + 0.5;
    let (x0, _jd_epoch, dt) = sv_from_coe(&sorted_sats, t_end);
    //println!("State Vector = {:?}", x0);

    //Propagate every satellite to t_end, 60 s step, one thread per core
    let t_prop = std::time::Instant::now();
    let orbits = propagate_all(&x0, &dt, &sorted_sats, 60.0);
    println!("Propagated {} orbits, {} columns each, in {:.2} s", orbits.len(), orbits[0].ncols(), t_prop.elapsed().as_secs_f64());
    std::fs::write("ORBIT_DATA.json", serde_json::to_string(&orbits)?)?;

    //Station (IP lookup, falls back to the constants in geodesy.rs) and the current time
    let station = geodesy::get_station();
    let jd_now = geodesy::now_jd();
    println!("Station {}  lat {:.4}  lon {:.4}  now {}", station.name, station.lat_deg, station.lon_deg, geodesy::jd_to_local_string(jd_now));

    //Every pass above 5 degrees, for every satellite, from now to the end of the data
    //Sky window: VIEW_REGION.json from the viewer if present, otherwise the whole sky above 5 degrees
    let region = passes::ViewRegion::load("VIEW_REGION.json").unwrap_or_else(|| passes::ViewRegion::full_sky(5.0));
    println!("View region: {}  az {:.0}->{:.0}  el {:.0}..{:.0}", region.name, region.az_from, region.az_to, region.el_min, region.el_max);
    let all_passes = passes::find_passes(&orbits, &sorted_sats, 60.0, &station, &region, jd_now);
    println!("Passes found = {}", all_passes.len());

    //Rank the passes in progress now or starting within 15 minutes
    let names = score::load_names("ELSET.json");
    let weights = score::Weights::standard();
    let ranks = score::rank_passes(&all_passes, &names, &transmitters, &weights, jd_now, 15.0);
    score::print_ranks(&ranks, 10);

    //Look angle tables (az / el / range vs time) for every ranked pass, best first
    let mut tracks: Vec<passes::LookAngleTrack> = Vec::new();
    for r in &ranks {
        tracks.push(passes::look_angle_track(&orbits[r.pass.column], &sorted_sats, 60.0, &station, &r.pass, &r.name));
    }
    std::fs::write("LOOK_ANGLES.json", serde_json::to_string_pretty(&tracks)?)?;

    //Settings + entries together, so the viewer can show how each score was built
    let report = score::report(ranks, &weights, &station, &region, 15.0, jd_now);
    std::fs::write("SATELLITE_RANKS.json", serde_json::to_string_pretty(&report)?)?;

    Ok(())

} 


//Categories-only mode: SORTED_SATS.json (which satellites) + NORADs.json (services) + ELSET.json (names)
//from the last full run, plus a fresh pull of CelesTrak's groups. Writes CATEGORIES.json.
fn categories_only() -> Result<(), Box<dyn std::error::Error>> {
    let sorted_sats: ElSetMatrix = serde_json::from_str(&std::fs::read_to_string("SORTED_SATS.json")?)?;
    let transmitters: Vec<satnogs::Transmitter> = serde_json::from_str(&std::fs::read_to_string("NORADs.json")?)?;
    let tracked: Vec<u32> = (0..sorted_sats.ncols()).map(|c| sorted_sats[(0, c)] as u32).collect();
    let cats = categories::build(&tracked, &transmitters, true)?;
    categories::write(&cats)
}

//Rank-only mode: load ORBIT_DATA.json / SORTED_SATS.json / NORADs.json written by a full run and
//redo just the pass finding + ranking for the current time. Takes a second instead of a minute.
fn rank_only() -> Result<(), Box<dyn std::error::Error>> {

    let sorted_sats: ElSetMatrix = serde_json::from_str(&std::fs::read_to_string("SORTED_SATS.json")?)?;
    let orbits: Vec<Matrix6xX<f64>> = serde_json::from_str(&std::fs::read_to_string("ORBIT_DATA.json")?)?;
    let transmitters: Vec<satnogs::Transmitter> = serde_json::from_str(&std::fs::read_to_string("NORADs.json")?)?;
    println!("Loaded {} orbits from disk (rank only)", orbits.len());

    let station = geodesy::get_station();
    let jd_now = geodesy::now_jd();
    println!("Station {}  lat {:.4}  lon {:.4}  now {}", station.name, station.lat_deg, station.lon_deg, geodesy::jd_to_local_string(jd_now));

    //Sky window: VIEW_REGION.json from the viewer if present, otherwise the whole sky above 5 degrees
    let region = passes::ViewRegion::load("VIEW_REGION.json").unwrap_or_else(|| passes::ViewRegion::full_sky(5.0));
    println!("View region: {}  az {:.0}->{:.0}  el {:.0}..{:.0}", region.name, region.az_from, region.az_to, region.el_min, region.el_max);
    let all_passes = passes::find_passes(&orbits, &sorted_sats, 60.0, &station, &region, jd_now);
    println!("Passes found = {}", all_passes.len());

    let names = score::load_names("ELSET.json");
    let weights = score::Weights::standard();
    let ranks = score::rank_passes(&all_passes, &names, &transmitters, &weights, jd_now, 15.0);
    score::print_ranks(&ranks, 10);

    let mut tracks: Vec<passes::LookAngleTrack> = Vec::new();
    for r in &ranks {
        tracks.push(passes::look_angle_track(&orbits[r.pass.column], &sorted_sats, 60.0, &station, &r.pass, &r.name));
    }
    std::fs::write("LOOK_ANGLES.json", serde_json::to_string_pretty(&tracks)?)?;

    //Settings + entries together, so the viewer can show how each score was built
    let report = score::report(ranks, &weights, &station, &region, 15.0, jd_now);
    std::fs::write("SATELLITE_RANKS.json", serde_json::to_string_pretty(&report)?)?;

    //The data ends at the propagation's t_end; warn when a fresh full run is due
    let data_end_jd = sorted_sats[(1, 0)] + (orbits[0].ncols() as f64 - 1.0) * 60.0 / 86400.0;
    let hours_left = (data_end_jd - jd_now) * 24.0;
    if hours_left < 2.0 {
        println!("WARNING: only {:.1} h of propagated data left, run a full `perigee` soon", hours_left);
    }

    Ok(())
}
