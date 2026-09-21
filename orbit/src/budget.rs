/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Propagator: the same RK4 integration, done a little at a time.
///
/// A live app cannot stop for seconds while 950 satellites are integrated from their epochs, so this keeps
/// every satellite's current state and lets the caller say "spend at most 50 ms on propagation now".
/// Each call to `run` advances satellites until the deadline, on as many threads as asked. Satellites
/// that have reached their target are handed out by `take_ready` as ordinary 6 x M matrices, so the
/// rest of an app never knows the difference between these and a file written by the engine.
///
/// Memory: a satellite is integrated from its element epoch (which can be a week old), but only the
/// states from `store_from_jd` onwards are kept. The columns handed out therefore start at a
/// "rebased" epoch: epoch + store_from_step * h. `take_ready` returns that epoch with the matrix.
/// The grid is unchanged (every stored column is a whole number of steps after the true epoch).
/////////////////////////////////////////////////////////////////////////////////////////////////////////
use nalgebra::{Vector6, Matrix6xX};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use crate::ElSetMatrix;
use crate::coe::state_from_coe;
use crate::rk4::rk4_step;

//How many RK4 steps to take between deadline checks (Instant::now is not free)
const STEPS_PER_CHECK: usize = 32;

//Work through satellites in order until each is at its target or the deadline passes. Returns steps taken.
fn work_until<'a>(sats: impl Iterator<Item = &'a mut SatTrack>, deadline: Instant, h: f64) -> usize {
    let mut done = 0;
    for sat in sats {
        while sat.needs_work() {
            if Instant::now() >= deadline { return done; }
            done += sat.advance(STEPS_PER_CHECK, h);
        }
    }
    done
}

pub struct SatTrack {
    y: Vector6<f64>,           // state at `step` steps after the epoch
    step: usize,               // steps taken so far (t = step * h seconds since epoch)
    target: usize,             // step to reach (inclusive)
    bstar: f64,
    epoch_jd: f64,             // true element epoch
    store_from: usize,         // first step index that is kept in `cols`
    cols: Vec<Vector6<f64>>,   // cols[k] = state at step store_from + k
    dirty: bool,               // new columns since the last take_ready
}

impl SatTrack {
    fn needs_work(&self) -> bool { self.step < self.target }

    //Up to n steps, stopping at the target. Returns the steps actually taken.
    fn advance(&mut self, n: usize, h: f64) -> usize {
        let mut done = 0;
        while done < n && self.step < self.target {
            self.y = rk4_step(self.y, self.step as f64 * h, h, self.bstar);
            self.step += 1;
            done += 1;
            if self.step >= self.store_from { self.cols.push(self.y); self.dirty = true; }
        }
        done
    }

    //JD of the last stored column, if any
    fn end_jd(&self, h: f64) -> Option<f64> {
        if self.cols.is_empty() { None } else { Some(self.epoch_jd + (self.store_from + self.cols.len() - 1) as f64 * h / 86400.0) }
    }
}

pub struct Propagator {
    pub h: f64,
    sats: Vec<SatTrack>,
}

impl Propagator {
    //One track per column of the element set matrix. States from store_from_jd on are kept; every
    //satellite is aimed at target_jd. Nothing is integrated here: that happens in `run`.
    pub fn new(coe: &ElSetMatrix, h: f64, store_from_jd: f64, target_jd: f64) -> Self {
        let mut sats = Vec::with_capacity(coe.ncols());
        for col in 0..coe.ncols() {
            let epoch_jd = coe[(1, col)];
            let store_from = (((store_from_jd - epoch_jd) * 86400.0 / h).floor().max(0.0)) as usize;
            let y = state_from_coe(coe, col);
            let mut cols = Vec::new();
            if store_from == 0 { cols.push(y); }
            sats.push(SatTrack { y, step: 0, target: 0, bstar: coe[(8, col)], epoch_jd, store_from, cols, dirty: store_from == 0 });
        }
        let mut p = Propagator { h, sats };
        p.set_target_jd(target_jd);
        p
    }

    pub fn len(&self) -> usize { self.sats.len() }
    pub fn is_empty(&self) -> bool { self.sats.is_empty() }

    //Aim every satellite at target_jd (never backwards). Steps needed = ceil((target - epoch) / h).
    pub fn set_target_jd(&mut self, target_jd: f64) {
        for s in &mut self.sats {
            let want = (((target_jd - s.epoch_jd) * 86400.0 / self.h).ceil().max(0.0)) as usize;
            if want > s.target { s.target = want; }
        }
    }

    //How many satellites still have steps to take
    pub fn pending(&self) -> usize { self.sats.iter().filter(|s| s.needs_work()).count() }

    //Has this satellite been integrated all the way to its target at least once?
    pub fn is_ready(&self, i: usize) -> bool { self.sats.get(i).is_some_and(|s| !s.needs_work() && !s.cols.is_empty()) }

    //Earliest end (JD of the last stored column) over the satellites that have columns: the time the
    //whole set is good to. None until the first satellite has stored something.
    pub fn min_end_jd(&self) -> Option<f64> {
        self.sats.iter().filter(|s| !s.needs_work()).filter_map(|s| s.end_jd(self.h)).fold(None, |m, e| Some(m.map_or(e, |m: f64| m.min(e))))
    }

    //Integrate until `budget` has elapsed or nothing is left, on `threads` threads (0 = one per core).
    //Returns the number of RK4 steps taken. The satellite list is split into one slice per thread with
    //`chunks_mut`: each thread gets exclusive &mut access to its own satellites, so no locks are needed,
    //and the scope makes sure all threads are done before `self` is touched again.
    pub fn run(&mut self, budget: Duration, threads: usize) -> usize {
        let pending = self.pending();
        if pending == 0 { return 0; }
        let deadline = Instant::now() + budget;
        let h = self.h;
        let threads = if threads == 0 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) } else { threads };
        let threads = threads.max(1).min(pending);

        if threads == 1 { return work_until(self.sats.iter_mut(), deadline, h); }

        //Only the satellites with work left are worth a thread's time: give each thread a slice of those
        let mut todo: Vec<&mut SatTrack> = self.sats.iter_mut().filter(|s| s.needs_work()).collect();
        let per = todo.len().div_ceil(threads);
        let total = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for chunk in todo.chunks_mut(per) {
                let total = &total;
                s.spawn(move || {
                    let done = work_until(chunk.iter_mut().map(|sat| &mut **sat), deadline, h);
                    total.fetch_add(done, Ordering::Relaxed);
                });
            }
        });
        total.load(Ordering::Relaxed)
    }

    //Every satellite that has reached its target and has new columns since the last call:
    //(column index, rebased epoch JD of the first stored column, 6 x M matrix). Clears the dirty flags.
    pub fn take_ready(&mut self) -> Vec<(usize, f64, Matrix6xX<f64>)> {
        let h = self.h;
        let mut out = Vec::new();
        for (i, s) in self.sats.iter_mut().enumerate() {
            if s.dirty && !s.needs_work() && !s.cols.is_empty() {
                s.dirty = false;
                let epoch = s.epoch_jd + s.store_from as f64 * h / 86400.0;
                out.push((i, epoch, Matrix6xX::from_columns(&s.cols)));
            }
        }
        out
    }
}
