/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Satellite categories (WEATHER, MILITARY, ...) for the viewer's TYPE filter.
///
/// Space-Track says every tracked object is a PAYLOAD and nothing more, so the type has to come from
/// elsewhere. Three sources, strongest first:
///   1. CelesTrak's curated groups (weather, military, gps-ops, ...), fetched one small JSON per group
///   2. SatNOGS transmitter service classes (Meteorological, Radionavigational, Amateur, ...)
///   3. Name rules for what the other two miss (NOAA, USA, COSMOS, STARLINK, ...)
/// A satellite may land in more than one category. Only the tracked set is written, to CATEGORIES.json,
/// which tv/server.py serves to the TV as categories.json. Run `perigee categories` to rebuild it alone.
/// CelesTrak asks not to pull the same file more than once every two hours; Perigee only fetches on a
/// full run or on `perigee categories`, both started by hand.
/////////////////////////////////////////////////////////////////////////////////////////////////////////
use std::collections::{BTreeSet, HashMap, HashSet};
use serde::Serialize;
use crate::satnogs::Transmitter;
type Error = Box<dyn std::error::Error>;

//(category, CelesTrak groups, SatNOGS services, name prefixes) in display order
const CATEGORIES: &[(&str, &[&str], &[&str], &[&str])] = &[
    ("WEATHER",    &["weather", "goes"],
                   &["Meteorological"],
                   &["NOAA", "METEOR", "FENGYUN", "FY-", "GOES", "METOP", "DMSP", "HIMAWARI", "ELEKTRO", "ARKTIKA", "GEO-KOMPSAT", "INSAT-3D", "SUOMI", "JPSS"]),
    ("EARTH OBS",  &["resource", "sarsat", "dmc", "planet", "spire", "argos"],
                   &["Earth Exploration"],
                   &["ICEYE", "JILIN", "GAOFEN", "CARTOSAT", "HAIYANG", "LEMUR", "SENTINEL", "LANDSAT", "TERRA", "AQUA", "SKYSAT", "FLOCK",
                     "CAPELLA", "UMBRA", "ZY-", "HJ-", "GRUS", "ZHUHAI", "SUPERVIEW", "WORLDVIEW", "PLEIADES", "SPOT", "RADARSAT", "TERRASAR",
                     "TANDEM", "ALOS", "RESOURCESAT", "RISAT", "OCEANSAT", "EOS-", "CBERS", "AMAZONIA", "KOMPSAT", "DEIMOS", "SAOCOM", "PAZ",
                     "SWOT", "ICESAT", "SMAP", "GRACE", "CFOSAT", "SEASAT", "TIANHUI", "SHIYAN", "WILDFIRE", "HERMES", "TAIJING", "BRO-"]),
    ("COMMS",      &["geo", "intelsat", "ses", "eutelsat", "telesat", "iridium-NEXT", "orbcomm", "globalstar", "x-comm", "other-comm", "tdrss"],
                   &["Mobile", "Maritime", "Aeronautical", "Inter-satellite", "Fixed", "Broadcasting"],
                   &["STARLINK", "ONEWEB", "KUIPER", "GONETS", "KINEIS", "ORBCOMM", "IRIDIUM", "GLOBALSTAR", "SPACEMOBILE", "BLUEBIRD", "CONNECTA",
                     "TIANQI", "INMARSAT", "INTELSAT", "EUTELSAT", "SES-", "TELESAT", "THURAYA", "VIASAT", "ECHOSTAR", "SIRIUS", "XM-", "TDRS",
                     "LUCH", "YAMAL", "EXPRESS", "TIANTONG", "APRIZESAT", "ION ", "HERMES", "SWARM", "LACUNA", "MYRIOTA", "ASTROCAST", "OMNISPACE"]),
    ("AMATEUR",    &["amateur"],
                   &["Amateur"],
                   &["AO-", "FO-", "SO-", "XW-", "CAS-", "OSCAR", "FUNCUBE", "AMSAT", "JAISAT", "TEVEL", "LILACSAT", "BY70", "UVSQ", "GREENCUBE"]),
    ("NAVIGATION", &["gnss", "gps-ops", "glo-ops", "galileo", "beidou", "sbas", "nnss", "musson"],
                   &["Radionavigational"],
                   &["NAVSTAR", "GPS", "GLONASS", "BEIDOU", "GALILEO", "GSAT0", "QZS", "IRNSS", "NVS-", "CENTISPACE", "WAAS", "EGNOS", "GAGAN", "MSAS", "SDCM"]),
    ("SCIENCE",    &["science", "geodetic", "engineering", "education"],
                   &["Space Research"],
                   &["HUBBLE", "HST", "CHANDRA", "FERMI", "SWIFT", "NUSTAR", "IXPE", "TESS", "XMM", "INTEGRAL", "GAIA", "EUCLID", "JWST", "SOHO",
                     "ACE", "WIND", "MMS", "THEMIS", "VAN ALLEN", "PROBA", "SWARM", "LAGEOS", "JASON", "TOPEX", "IONOSFERA", "SPEKTR", "TIANWEN",
                     "XPNAV", "HXMT", "DAMPE", "QUESS", "MICIUS", "TAIJI", "TIANQIN", "ASTROSAT", "XPOSAT", "CSES", "CUTE", "SPARCS"]),
    ("MILITARY",   &["military", "radar"],
                   &[],
                   &["USA ", "COSMOS", "YAOGAN", "OPS ", "NROL", "LKW", "TJS", "SBIRS", "WGS", "MUOS", "AEHF", "MILSTAR", "DSP", "NOSS", "KH-",
                     "SAR-LUPE", "SARAH", "HELIOS", "CSO-", "CERES", "SYRACUSE", "SKYNET", "SICRAL", "COSMO-SKYMED", "OFEQ", "TECSAR", "KOMPSAT-5",
                     "TIANLIAN", "SHIJIAN", "SJ-", "CHUANGXIN", "STPSAT", "XSS", "GSSAP", "LDPE", "NAVY", "MENTOR", "TRUMPET", "ORION", "LACROSSE",
                     "ONYX", "TOPAZ", "FIA", "MISTY", "QUASAR", "SDS", "IMPROVED CRYSTAL", "GEO-1", "SPACE BASED", "TITAN", "BIRD ", "RISAT-2"]),
    ("STATIONS",   &["stations"],
                   &[],
                   &["ISS", "CSS", "TIANHE", "WENTIAN", "MENGTIAN", "TIANGONG", "PROGRESS", "SOYUZ", "DRAGON", "CYGNUS", "SHENZHOU", "TIANZHOU", "HTV"]),
    ("CUBESATS",   &["cubesat"],
                   &[],
                   &[]),
];

const CELESTRAK: &str = "https://celestrak.org/NORAD/elements/gp.php?GROUP={g}&FORMAT=json";

#[derive(Serialize)]
pub struct Category { pub name: String, pub norads: Vec<u32> }

#[derive(Serialize)]
pub struct Categories {
    pub built_utc: String,
    pub celestrak_groups: usize,   // groups that answered (0 = offline: services + names only)
    pub tracked: usize,
    pub unclassified: usize,
    pub categories: Vec<Category>,
}

//NORAD -> OBJECT_NAME from the raw Space-Track download on disk (get_sat_data writes it before this runs)
fn names_from_elset() -> HashMap<u32, String> {
    let mut names = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string("ELSET.json") {
        if let Ok(recs) = serde_json::from_str::<Vec<serde_json::Value>>(&txt) {
            for r in recs {
                let id = r["NORAD_CAT_ID"].as_str().and_then(|s| s.parse::<u32>().ok()).or_else(|| r["NORAD_CAT_ID"].as_u64().map(|v| v as u32));
                if let (Some(id), Some(n)) = (id, r["OBJECT_NAME"].as_str()) { names.insert(id, n.to_uppercase()); }
            }
        }
    }
    names
}

//One CelesTrak group -> the NORAD IDs in it. CelesTrak's JSON carries NORAD_CAT_ID as a number.
fn fetch_group(client: &reqwest::blocking::Client, group: &str) -> Result<BTreeSet<u32>, Error> {
    let body = client.get(CELESTRAK.replace("{g}", group)).send()?.text()?;
    let recs: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|_| format!("{}", body.lines().next().unwrap_or("bad reply")))?;
    Ok(recs.iter().filter_map(|r| r["NORAD_CAT_ID"].as_u64().map(|v| v as u32).or_else(|| r["NORAD_CAT_ID"].as_str().and_then(|s| s.parse().ok()))).collect())
}

pub fn build(tracked: &[u32], transmitters: &[Transmitter], fetch_celestrak: bool) -> Result<Categories, Error> {

    let tracked_set: HashSet<u32> = tracked.iter().copied().collect();
    let names = names_from_elset();

    //NORAD -> SatNOGS service classes of its transmitters
    let mut services: HashMap<u32, HashSet<&str>> = HashMap::new();
    for t in transmitters {
        if let Some(id) = t.norad_cat_id { services.entry(id).or_default().insert(t.service.as_str()); }
    }

    //CelesTrak groups, each fetched once even if several categories use it
    let mut groups: HashMap<&str, BTreeSet<u32>> = HashMap::new();
    if fetch_celestrak {
        let client = reqwest::blocking::Client::builder().user_agent("perigee/0.1 (satellite tracker)").timeout(std::time::Duration::from_secs(60)).build()?;
        let wanted: BTreeSet<&str> = CATEGORIES.iter().flat_map(|c| c.1.iter().copied()).collect();
        for g in wanted {
            match fetch_group(&client, g) {
                Ok(set) => { println!("CelesTrak {:<14} {:>5} objects, {:>3} tracked", g, set.len(), set.iter().filter(|i| tracked_set.contains(i)).count()); groups.insert(g, set); }
                Err(e) => println!("CelesTrak {:<14} skipped: {}", g, e),
            }
        }
    }

    let mut classified: HashSet<u32> = HashSet::new();
    let mut categories = Vec::new();
    for (name, cel_groups, svc, prefixes) in CATEGORIES {
        let mut members: BTreeSet<u32> = BTreeSet::new();
        for g in *cel_groups {
            if let Some(set) = groups.get(g) { members.extend(set.iter().filter(|i| tracked_set.contains(i))); }
        }
        for &id in tracked {
            let by_service = services.get(&id).is_some_and(|s| svc.iter().any(|w| s.contains(w)));
            let by_name = names.get(&id).is_some_and(|n| prefixes.iter().any(|p| n.starts_with(p)));
            if by_service || by_name { members.insert(id); }
        }
        classified.extend(members.iter());
        categories.push(Category { name: name.to_string(), norads: members.into_iter().collect() });
    }

    Ok(Categories {
        built_utc: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        celestrak_groups: groups.len(),
        tracked: tracked.len(),
        unclassified: tracked.iter().filter(|i| !classified.contains(i)).count(),
        categories,
    })
}

pub fn write(cats: &Categories) -> Result<(), Error> {
    std::fs::write("CATEGORIES.json", serde_json::to_string_pretty(cats)?)?;
    for c in &cats.categories { println!("  {:<11} {:>4}", c.name, c.norads.len()); }
    println!("CATEGORIES.json: {} tracked, {} unclassified, {} CelesTrak groups", cats.tracked, cats.unclassified, cats.celestrak_groups);
    Ok(())
}
