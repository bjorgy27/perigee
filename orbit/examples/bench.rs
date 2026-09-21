//Timing: sequential vs parallel propagate_all, and the budgeted Propagator, on a saved SORTED_SATS.json.
//    cargo run --release --example bench -- ../src/SORTED_SATS.json
use perigee_orbit::{sv_from_coe, propagate_all_threads, ElSetMatrix, Propagator};
use std::time::{Duration, Instant};

fn now_jd() -> f64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    d.as_secs_f64() / 86400.0 + 2440587.5
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "src/SORTED_SATS.json".into());
    let coe: ElSetMatrix = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    let t_end = now_jd() + 0.5;
    let (x0, epochs, dt) = sv_from_coe(&coe, t_end);
    let steps: f64 = dt.iter().map(|d| (d / 60.0).ceil()).sum();
    let oldest = dt.iter().cloned().fold(0.0, f64::max) / 3600.0;
    println!("{} satellites, {:.0} RK4 steps in total, oldest epoch {:.0} h ago, target now + 12 h", coe.ncols(), steps, oldest - 12.0);

    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    for &th in &[1usize, cores] {
        let t = Instant::now();
        let orbits = propagate_all_threads(&x0, &dt, &coe, 60.0, th);
        let el = t.elapsed();
        let cols: usize = orbits.iter().map(|m| m.ncols()).sum();
        println!("propagate_all  threads={:<2} {:>8.3} s   {} columns  ({:.2} us/step)", th, el.as_secs_f64(), cols, el.as_secs_f64() * 1e6 / steps);
    }

    //Where the time really goes: the JSON the engine writes and the viewer used to read
    let orbits = propagate_all_threads(&x0, &dt, &coe, 60.0, cores);
    let t = Instant::now();
    let js = serde_json::to_string(&orbits).unwrap();
    let tw = t.elapsed();
    let t = Instant::now();
    let back: Vec<nalgebra::Matrix6xX<f64>> = serde_json::from_str(&js).unwrap();
    println!("ORBIT_DATA.json  {:.0} MB: serialize {:.2} s, parse {:.2} s  ({} matrices)", js.len() as f64 / 1e6, tw.as_secs_f64(), t.elapsed().as_secs_f64(), back.len());
    drop(js); drop(back); drop(orbits);

    //Budgeted: only keep from now - 45 min, 50 ms slices, all cores
    let store_from = now_jd() - 45.0 / 1440.0;
    let mut p = Propagator::new(&coe, 60.0, store_from, t_end);
    let t = Instant::now();
    let mut slices = 0; let mut ready = 0;
    while p.pending() > 0 {
        p.run(Duration::from_millis(50), 0);
        ready += p.take_ready().len();
        slices += 1;
    }
    println!("Propagator     50 ms slices: {} slices, {:.3} s wall, {} satellites published, coverage to {:.2} h from now",
             slices, t.elapsed().as_secs_f64(), ready, (p.min_end_jd().unwrap() - now_jd()) * 24.0);
    let _ = epochs;
}
