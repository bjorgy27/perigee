/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Geodesy
/// Station position on the WGS-84 ellipsoid, Greenwich sidereal time, inertial -> Earth-fixed,
/// and look angles (azimuth / elevation / range) from the station to a satellite.
/// Vallado: Algorithm 15 (sidereal time), Algorithm 51 (site vector), section 4.4 (look angles)
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
use nalgebra::Vector3;
use serde::Serialize;

//The station. Latitude north positive, longitude east positive (west is negative).
//Home: The Napier, N Williamson Blvd, Daytona Beach (OpenStreetMap house point for the address).
const STATION_NAME: &str = "THE NAPIER";
const STATION_LAT_DEG: f64 = 29.2452787;
const STATION_LON_DEG: f64 = -81.1031710;
const STATION_ALT_M: f64 = 10.0;
const AUTO_LOCATE: bool = false;   // true: try an IP lookup first (city-level accuracy)

//WGS-84 ellipsoid
const WGS84_A: f64 = 6378.137;               // km
const WGS84_F: f64 = 1.0 / 298.257223563;

const SN_IP_URL: &str = "https://ipinfo.io/json";



#[derive(Serialize, Debug, Clone)]
pub struct Station {
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct LookAngle {
    pub az_deg: f64,      // from north, clockwise
    pub el_deg: f64,      // above the horizon
    pub range_km: f64,    // straight line distance
}



//Station from the IP lookup (city level accuracy), falling back to the constants above
pub fn get_station() -> Station {

    let fallback = Station {
        name: STATION_NAME.to_string(),
        lat_deg: STATION_LAT_DEG,
        lon_deg: STATION_LON_DEG,
        alt_m: STATION_ALT_M,
    };

    if !AUTO_LOCATE {
        return fallback;
    }

    let response = match reqwest::blocking::get(SN_IP_URL) {
        Ok(r) => r,
        Err(_) => return fallback,
    };

    let json: serde_json::Value = match response.json() {
        Ok(j) => j,
        Err(_) => return fallback,
    };

    //"loc" is "lat,lon" as one string
    let loc = match json["loc"].as_str() {
        Some(s) => s.to_string(),
        None => return fallback,
    };

    let parts: Vec<&str> = loc.split(',').collect();
    if parts.len() != 2 {
        return fallback;
    }

    let lat: f64 = match parts[0].trim().parse() { Ok(v) => v, Err(_) => return fallback };
    let lon: f64 = match parts[1].trim().parse() { Ok(v) => v, Err(_) => return fallback };

    let city = json["city"].as_str().unwrap_or("").to_string();
    let region = json["region"].as_str().unwrap_or("").to_string();
    let name = if city.is_empty() { STATION_NAME.to_string() } else { format!("{city}, {region}") };

    Station { name, lat_deg: lat, lon_deg: lon, alt_m: STATION_ALT_M }
}



//Current time as a Julian date (UTC)
pub fn now_jd() -> f64 {
    let now = chrono::Utc::now();
    let unix_seconds = now.timestamp() as f64 + now.timestamp_subsec_micros() as f64 * 1e-6;
    unix_seconds / 86400.0 + 2440587.5
}

//Julian date -> "2026-09-19 02:28:43 UTC"
pub fn jd_to_utc_string(jd: f64) -> String {
    let unix_seconds = (jd - 2440587.5) * 86400.0;
    match chrono::DateTime::from_timestamp(unix_seconds.floor() as i64, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => "----".to_string(),
    }
}

//Julian date -> Eastern time, "2026-09-18 22:28:43 EDT" (switches to EST automatically in winter)
pub fn jd_to_local_string(jd: f64) -> String {
    let unix_seconds = (jd - 2440587.5) * 86400.0;
    match chrono::DateTime::from_timestamp(unix_seconds.floor() as i64, 0) {
        Some(dt) => dt.with_timezone(&chrono_tz::America::New_York).format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        None => "----".to_string(),
    }
}



//Greenwich mean sidereal time in radians (Vallado Algorithm 15, UTC used as UT1)
pub fn gmst(jd: f64) -> f64 {
    let t = (jd - 2451545.0) / 36525.0;
    let seconds = 67310.54841
        + (876600.0 * 3600.0 + 8640184.812866) * t
        + 0.093104 * t * t
        - 6.2e-6 * t * t * t;
    let degrees = seconds.rem_euclid(86400.0) / 240.0;
    degrees.to_radians()
}



//Station position in the Earth-fixed frame (km)
pub fn station_ecef(station: &Station) -> Vector3<f64> {
    let lat = station.lat_deg.to_radians();
    let lon = station.lon_deg.to_radians();
    let h = station.alt_m / 1000.0;

    let e2 = WGS84_F * (2.0 - WGS84_F);
    let n = WGS84_A / (1.0 - e2 * lat.sin() * lat.sin()).sqrt();

    let x = (n + h) * lat.cos() * lon.cos();
    let y = (n + h) * lat.cos() * lon.sin();
    let z = (n * (1.0 - e2) + h) * lat.sin();

    Vector3::new(x, y, z)
}



//Inertial (TEME) -> Earth-fixed: rotate about the pole by the sidereal angle
pub fn eci_to_ecef(r: Vector3<f64>, gmst_rad: f64) -> Vector3<f64> {
    let c = gmst_rad.cos();
    let s = gmst_rad.sin();

    let x =  c * r[0] + s * r[1];
    let y = -s * r[0] + c * r[1];
    let z = r[2];

    Vector3::new(x, y, z)
}



//Azimuth, elevation and range from the station to a satellite at inertial position sat_eci (km) at time jd
pub fn look_angles(station: &Station, sat_eci: Vector3<f64>, jd: f64) -> LookAngle {

    let sat_ecef = eci_to_ecef(sat_eci, gmst(jd));
    let d = sat_ecef - station_ecef(station);

    let lat = station.lat_deg.to_radians();
    let lon = station.lon_deg.to_radians();

    //Rotate the station -> satellite vector into East / North / Up
    let east  = -lon.sin() * d[0] + lon.cos() * d[1];
    let north = -lat.sin() * lon.cos() * d[0] - lat.sin() * lon.sin() * d[1] + lat.cos() * d[2];
    let up    =  lat.cos() * lon.cos() * d[0] + lat.cos() * lon.sin() * d[1] + lat.sin() * d[2];

    let range = (east * east + north * north + up * up).sqrt();
    let el = (up / range).asin().to_degrees();
    let az = east.atan2(north).to_degrees().rem_euclid(360.0);

    LookAngle { az_deg: az, el_deg: el, range_km: range }
}
