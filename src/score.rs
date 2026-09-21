/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Score
/// Rank passes: which satellites are worth pointing the dish at in the next few minutes.
/// score = w_duration * duration_term + w_elevation * elevation_term
///       + w_transmitter * transmitter_term + w_freshness * freshness_term        (each term 0..1)
/// The pass is judged by what is still ahead of it at jd_now: minutes left above the mask, and the
/// best elevation still to come (the peak if it has not happened yet, the current elevation if it has).
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
use std::collections::HashMap;
use serde::Serialize;
use crate::passes::{Pass, ViewRegion};
use crate::satnogs::Transmitter;
use crate::geodesy::{Station, jd_to_utc_string, jd_to_local_string};



#[derive(Serialize, Clone, Debug)]
pub struct Weights {
    pub duration: f64,
    pub elevation: f64,
    pub transmitter: f64,
    pub freshness: f64,
}

impl Weights {
    pub fn standard() -> Weights {
        Weights { duration: 0.35, elevation: 0.30, transmitter: 0.25, freshness: 0.10 }
    }
}

//Transmitter details copied into the output (the SatNOGS struct itself is deserialize-only)
#[derive(Serialize, Debug, Clone)]
pub struct TxInfo {
    pub description: String,
    pub downlink_low_hz: Option<u64>,
    pub downlink_high_hz: Option<u64>,
    pub mode: Option<String>,
    pub baud: Option<f64>,
    pub service: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct RankedSat {
    pub rank: usize,
    pub score: f64,
    pub norad_id: u32,
    pub name: String,
    pub in_progress: bool,          // already above the mask right now
    pub minutes_until_aos: f64,     // negative when in progress
    pub minutes_left: f64,          // time still above the mask from now (or the full pass if not started)
    pub minutes_to_peak: f64,       // negative when the peak has already passed
    pub el_now_deg: f64,            // 0 unless in progress
    pub best_el_ahead_deg: f64,     // what the elevation term was scored on
    pub aos_local: String,
    pub los_local: String,
    pub duration_min: f64,
    pub max_el_deg: f64,
    pub range_at_max_km: f64,
    pub epoch_age_h: f64,
    pub duration_term: f64,
    pub elevation_term: f64,
    pub transmitter_term: f64,
    pub freshness_term: f64,
    pub transmitters: Vec<TxInfo>,
    pub pass: Pass,
}



//NORAD ID -> OBJECT_NAME from the raw Space-Track download saved by get_sat_data
pub fn load_names(path: &str) -> HashMap<u32, String> {

    let mut names: HashMap<u32, String> = HashMap::new();

    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return names,
    };
    let records: Vec<serde_json::Value> = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(_) => return names,
    };

    for record in records {
        let id: Option<u32> = record["NORAD_CAT_ID"].as_str().and_then(|s| s.parse().ok());
        let name = record["OBJECT_NAME"].as_str();
        if let (Some(id), Some(name)) = (id, name) {
            names.insert(id, name.to_string());
        }
    }

    names
}



//More time still above the mask is better, saturating at 10 minutes.
//A pass with under 2 minutes left is not worth slewing to at all.
fn duration_term(minutes_left: f64) -> f64 {
    if minutes_left < 2.0 { 0.0 } else { (minutes_left / 10.0).min(1.0) }
}

//Higher is better (shorter range, less atmosphere), but a pass through the zenith is a keyhole
//the mount cannot follow, so anything above 85 degrees is knocked down
fn elevation_term(best_el_ahead_deg: f64) -> f64 {
    let term = (best_el_ahead_deg.to_radians()).sin();
    if best_el_ahead_deg > 85.0 { term * 0.3 } else { term }
}

//1.0 = at least one transmitter with a downlink, 0.5 = listed but no downlink frequency, 0 = nothing
fn transmitter_term(transmitters: &[TxInfo]) -> f64 {
    let mut best = 0.0;
    for tx in transmitters {
        let value = if tx.downlink_low_hz.is_some() { 1.0 } else { 0.5 };
        if value > best { best = value; }
    }
    best
}

//Fresh element sets predict better; a day old is worth nothing
fn freshness_term(epoch_age_h: f64) -> f64 {
    (1.0 - epoch_age_h / 24.0).max(0.0)
}



//Rank every pass that is in progress now or starts within horizon_min minutes
pub fn rank_passes(
    passes: &[Pass],
    names: &HashMap<u32, String>,
    transmitters: &[Transmitter],
    weights: &Weights,
    jd_now: f64,
    horizon_min: f64,
) -> Vec<RankedSat> {

    //Group transmitters by satellite once
    let mut by_sat: HashMap<u32, Vec<TxInfo>> = HashMap::new();
    for tx in transmitters {
        if let Some(id) = tx.norad_cat_id {
            let info = TxInfo {
                description: tx.description.clone(),
                downlink_low_hz: tx.downlink_low,
                downlink_high_hz: tx.downlink_high,
                mode: tx.mode.clone(),
                baud: tx.baud,
                service: tx.service.clone(),
            };
            by_sat.entry(id).or_insert_with(Vec::new).push(info);
        }
    }

    let jd_horizon = jd_now + horizon_min / 1440.0;
    let mut ranked: Vec<RankedSat> = Vec::new();

    for pass in passes {

        //Already over, or not starting soon enough
        if pass.los_jd <= jd_now || pass.aos_jd > jd_horizon {
            continue;
        }

        let txs = by_sat.get(&pass.norad_id).cloned().unwrap_or_default();
        let name = names.get(&pass.norad_id).cloned().unwrap_or_else(|| format!("NORAD {}", pass.norad_id));

        //What is still ahead of this pass right now
        let in_progress = pass.aos_jd <= jd_now;
        let minutes_left = (pass.los_jd - pass.aos_jd.max(jd_now)) * 1440.0;
        let minutes_to_peak = (pass.max_el_jd - jd_now) * 1440.0;
        let best_el_ahead = if minutes_to_peak >= 0.0 { pass.max_el_deg } else { pass.el_now_deg };

        let d = duration_term(minutes_left);
        let e = elevation_term(best_el_ahead);
        let t = transmitter_term(&txs);
        let f = freshness_term(pass.epoch_age_h);

        let score = weights.duration * d + weights.elevation * e + weights.transmitter * t + weights.freshness * f;

        ranked.push(RankedSat {
            rank: 0,
            score,
            norad_id: pass.norad_id,
            name,
            in_progress,
            minutes_until_aos: (pass.aos_jd - jd_now) * 1440.0,
            minutes_left,
            minutes_to_peak,
            el_now_deg: pass.el_now_deg,
            best_el_ahead_deg: best_el_ahead,
            aos_local: pass.aos_local.clone(),
            los_local: pass.los_local.clone(),
            duration_min: pass.duration_min,
            max_el_deg: pass.max_el_deg,
            range_at_max_km: pass.range_at_max_km,
            epoch_age_h: pass.epoch_age_h,
            duration_term: d,
            elevation_term: e,
            transmitter_term: t,
            freshness_term: f,
            transmitters: txs,
            pass: pass.clone(),
        });
    }

    //Best first
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    for i in 0..ranked.len() {
        ranked[i].rank = i + 1;
    }

    ranked
}



//What gets written to SATELLITE_RANKS.json: the settings the ranking was made with, then the entries
#[derive(Serialize, Debug)]
pub struct RankReport {
    pub generated_utc: String,
    pub generated_local: String,
    pub station_name: String,
    pub station_lat_deg: f64,
    pub station_lon_deg: f64,
    pub mask_deg: f64,
    pub horizon_min: f64,
    pub region: ViewRegion,
    pub weights: Weights,
    pub entries: Vec<RankedSat>,
}

pub fn report(entries: Vec<RankedSat>, weights: &Weights, station: &Station, region: &ViewRegion, horizon_min: f64, jd_now: f64) -> RankReport {
    let mask_deg = region.el_min;
    RankReport {
        region: region.clone(),
        generated_utc: jd_to_utc_string(jd_now),
        generated_local: jd_to_local_string(jd_now),
        station_name: station.name.clone(),
        station_lat_deg: station.lat_deg,
        station_lon_deg: station.lon_deg,
        mask_deg,
        horizon_min,
        weights: weights.clone(),
        entries,
    }
}



//Terminal summary of the top entries
pub fn print_ranks(ranked: &[RankedSat], top: usize) {
    println!("rank  score  NORAD   name                      AOS (local)              in      left   el now  peak   to peak");
    for r in ranked.iter().take(top) {
        let when = if r.in_progress { "  NOW ".to_string() } else { format!("{:5.1}m", r.minutes_until_aos) };
        let peak = if r.minutes_to_peak >= 0.0 { format!("{:5.1}m", r.minutes_to_peak) } else { " past ".to_string() };
        println!(
            "{:4}  {:.3}  {:6}  {:24}  {:23}  {}  {:5.1}m  {:5.1}   {:5.1}  {}",
            r.rank, r.score, r.norad_id, r.name, r.aos_local, when, r.minutes_left, r.el_now_deg, r.max_el_deg, peak
        );
    }
}
