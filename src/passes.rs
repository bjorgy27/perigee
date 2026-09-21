/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Passes
/// Walk every propagated trajectory, find when each satellite rises above the elevation mask (AOS)
/// and drops back below it (LOS), and build look angle tables (az / el / range vs time) for a pass.
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
use nalgebra::{Vector3, Matrix6xX};
use serde::{Serialize, Deserialize};
use crate::spacetrack::ElSetMatrix;
use crate::geodesy::{Station, LookAngle, look_angles, jd_to_utc_string, jd_to_local_string};



//A window of sky the dish can use: azimuth from az_from clockwise to az_to (degrees from north),
//elevation between el_min and el_max. The default is the whole sky above the elevation mask.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ViewRegion {
    pub name: String,
    pub az_from: f64,
    pub az_to: f64,
    pub el_min: f64,
    pub el_max: f64,
}

impl ViewRegion {
    pub fn full_sky(mask_deg: f64) -> ViewRegion {
        ViewRegion { name: "FULL SKY".to_string(), az_from: 0.0, az_to: 360.0, el_min: mask_deg, el_max: 90.0 }
    }

    //Is a look angle inside the window? Azimuth wraps: 315 -> 45 means north-ish.
    pub fn contains(&self, az_deg: f64, el_deg: f64) -> bool {
        if el_deg < self.el_min || el_deg > self.el_max {
            return false;
        }
        let span = (self.az_to - self.az_from).rem_euclid(360.0);
        if span == 0.0 || (self.az_to - self.az_from).abs() >= 360.0 {
            return true;   // all azimuths
        }
        let offset = (az_deg - self.az_from).rem_euclid(360.0);
        offset <= span
    }

    //Read VIEW_REGION.json written by the viewer; None if it is not there or not valid
    pub fn load(path: &str) -> Option<ViewRegion> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Pass {
    pub column: usize,          // column in sorted_sats / index in orbits
    pub norad_id: u32,
    pub aos_jd: f64,
    pub los_jd: f64,
    pub aos_utc: String,
    pub aos_local: String,
    pub los_utc: String,
    pub los_local: String,
    pub duration_min: f64,
    pub max_el_deg: f64,
    pub max_el_jd: f64,
    pub az_at_aos_deg: f64,
    pub az_at_los_deg: f64,
    pub range_at_max_km: f64,
    pub epoch_age_h: f64,       // how old the element set is at AOS
    pub el_now_deg: f64,        // elevation at jd_now if the pass is in progress, else 0
}

#[derive(Serialize, Debug, Clone)]
pub struct LookAngleSample {
    pub jd: f64,
    pub utc: String,
    pub local: String,
    pub az_deg: f64,
    pub el_deg: f64,
    pub range_km: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct LookAngleTrack {
    pub norad_id: u32,
    pub name: String,
    pub column: usize,
    pub step_seconds: f64,
    pub samples: Vec<LookAngleSample>,
}



//Absolute time (Julian date) of column c of satellite col: its epoch plus c steps
fn column_jd(sorted_sats: &ElSetMatrix, col: usize, c: usize, h: f64) -> f64 {
    sorted_sats[(1, col)] + (c as f64) * h / 86400.0
}

//Inertial position (km) stored in column c of a trajectory
fn column_position(orbit: &Matrix6xX<f64>, c: usize) -> Vector3<f64> {
    Vector3::new(orbit[(0, c)], orbit[(1, c)], orbit[(2, c)])
}

//Elevation at jd_now, interpolated between stored samples; 0 when the pass is not in progress
fn elevation_at(angles: &[LookAngle], sorted_sats: &ElSetMatrix, col: usize, h: f64, jd_now: f64, aos_jd: f64, los_jd: f64) -> f64 {
    if jd_now < aos_jd || jd_now > los_jd {
        return 0.0;
    }
    let f = (jd_now - sorted_sats[(1, col)]) * 86400.0 / h;
    let c0 = (f.floor().max(0.0) as usize).min(angles.len() - 1);
    let c1 = (c0 + 1).min(angles.len() - 1);
    let a = (f - c0 as f64).clamp(0.0, 1.0);
    angles[c0].el_deg + (angles[c1].el_deg - angles[c0].el_deg) * a
}

//Time between two samples where the satellite enters (entering = true) or leaves the region.
//Bisects on the position interpolated between the two stored columns.
fn region_crossing_jd(orbit: &Matrix6xX<f64>, c0: usize, jd_a: f64, jd_b: f64, station: &Station, region: &ViewRegion, entering: bool) -> f64 {
    let pa = column_position(orbit, c0);
    let pb = column_position(orbit, c0 + 1);
    let mut lo = 0.0;   // fraction of the way from a to b: known "outside" side when entering
    let mut hi = 1.0;
    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        let p = pa + (pb - pa) * mid;
        let jd = jd_a + (jd_b - jd_a) * mid;
        let la = look_angles(station, p, jd);
        let inside = region.contains(la.az_deg, la.el_deg);
        if inside == entering { hi = mid; } else { lo = mid; }
    }
    jd_a + (jd_b - jd_a) * 0.5 * (lo + hi)
}



//Every pass above mask_deg, for every satellite, that has not already ended at jd_now
pub fn find_passes(
    orbits: &[Matrix6xX<f64>],
    sorted_sats: &ElSetMatrix,
    h: f64,
    station: &Station,
    region: &ViewRegion,
    jd_now: f64,
) -> Vec<Pass> {

    let mut passes: Vec<Pass> = Vec::new();

    for col in 0..orbits.len() {

        let orbit = &orbits[col];
        let norad_id = sorted_sats[(0, col)] as u32;
        let epoch_jd = sorted_sats[(1, col)];

        //Look angles at every stored column
        let mut angles: Vec<LookAngle> = Vec::new();
        for c in 0..orbit.ncols() {
            let jd = column_jd(sorted_sats, col, c, h);
            angles.push(look_angles(station, column_position(orbit, c), jd));
        }

        //Walk the samples looking for the satellite entering / leaving the region
        let mut above = region.contains(angles[0].az_deg, angles[0].el_deg);
        let mut aos_jd = column_jd(sorted_sats, col, 0, h);
        let mut az_at_aos = angles[0].az_deg;
        let mut max_el = angles[0].el_deg;
        let mut max_el_jd = aos_jd;
        let mut range_at_max = angles[0].range_km;

        for c in 1..angles.len() {

            let jd_prev = column_jd(sorted_sats, col, c - 1, h);
            let jd = column_jd(sorted_sats, col, c, h);
            let el = angles[c].el_deg;

            let inside = region.contains(angles[c].az_deg, el);
            if !above && inside {
                //Entering the region: acquisition of signal
                above = true;
                aos_jd = region_crossing_jd(orbit, c - 1, jd_prev, jd, station, region, true);
                az_at_aos = angles[c].az_deg;
                max_el = el;
                max_el_jd = jd;
                range_at_max = angles[c].range_km;
            }
            else if above && !inside {
                //Leaving the region: loss of signal
                above = false;
                let los_jd = region_crossing_jd(orbit, c - 1, jd_prev, jd, station, region, false);

                if los_jd >= jd_now {
                    passes.push(Pass {
                        el_now_deg: elevation_at(&angles, sorted_sats, col, h, jd_now, aos_jd, los_jd),
                        column: col,
                        norad_id,
                        aos_jd,
                        los_jd,
                        aos_utc: jd_to_utc_string(aos_jd),
                        aos_local: jd_to_local_string(aos_jd),
                        los_utc: jd_to_utc_string(los_jd),
                        los_local: jd_to_local_string(los_jd),
                        duration_min: (los_jd - aos_jd) * 1440.0,
                        max_el_deg: max_el,
                        max_el_jd,
                        az_at_aos_deg: az_at_aos,
                        az_at_los_deg: angles[c - 1].az_deg,
                        range_at_max_km: range_at_max,
                        epoch_age_h: (aos_jd - epoch_jd) * 24.0,
                    });
                }
            }
            else if above && el > max_el {
                max_el = el;
                max_el_jd = jd;
                range_at_max = angles[c].range_km;
            }
        }
        //A pass still in progress at the end of the data is dropped: its LOS is unknown
    }

    passes
}



//Look angles at every stored column from one sample before AOS to one after LOS
pub fn look_angle_track(
    orbit: &Matrix6xX<f64>,
    sorted_sats: &ElSetMatrix,
    h: f64,
    station: &Station,
    pass: &Pass,
    name: &str,
) -> LookAngleTrack {

    let col = pass.column;
    let mut samples: Vec<LookAngleSample> = Vec::new();

    for c in 0..orbit.ncols() {
        let jd = column_jd(sorted_sats, col, c, h);
        if jd < pass.aos_jd - h / 86400.0 || jd > pass.los_jd + h / 86400.0 {
            continue;
        }
        let la = look_angles(station, column_position(orbit, c), jd);
        samples.push(LookAngleSample {
            jd,
            utc: jd_to_utc_string(jd),
            local: jd_to_local_string(jd),
            az_deg: la.az_deg,
            el_deg: la.el_deg,
            range_km: la.range_km,
        });
    }

    LookAngleTrack {
        norad_id: pass.norad_id,
        name: name.to_string(),
        column: col,
        step_seconds: h,
        samples,
    }
}
