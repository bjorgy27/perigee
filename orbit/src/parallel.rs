/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// propagate_all: every satellite from its own epoch to t_end, all cores busy.
///
/// Each satellite is independent, so this is the easy kind of parallelism: split the column range into
/// one chunk per thread, let each thread run the plain single-satellite `propagate` over its chunk, then
/// put the results back in column order. `std::thread::scope` lets the threads borrow x0 / dt / coe
/// directly: the scope guarantees every thread has finished before the borrows end, so no Arc, no clone.
/////////////////////////////////////////////////////////////////////////////////////////////////////////
use nalgebra::{Vector6, Matrix6xX};
use crate::ElSetMatrix;
use crate::rk4::propagate;

//Propagate every column of x0 from its own epoch for dt[col] seconds, step h, one thread per core.
//Returns one 6 x M trajectory matrix per satellite, in the same column order as coe.
pub fn propagate_all(x0: &Matrix6xX<f64>, dt: &[f64], coe: &ElSetMatrix, h: f64) -> Vec<Matrix6xX<f64>> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    propagate_all_threads(x0, dt, coe, h, threads)
}

//Same, with the thread count chosen by the caller (1 = the old sequential loop).
pub fn propagate_all_threads(x0: &Matrix6xX<f64>, dt: &[f64], coe: &ElSetMatrix, h: f64, threads: usize) -> Vec<Matrix6xX<f64>> {

    let n = x0.ncols();
    if n == 0 { return Vec::new(); }

    //One satellite's trajectory, by column index
    let one = |col: usize| -> Matrix6xX<f64> {
        let y0: Vector6<f64> = x0.column(col).into();
        let bstar = coe[(8, col)];
        propagate(y0, dt[col], h, bstar)
    };

    let threads = threads.max(1).min(n);
    if threads == 1 {
        return (0..n).map(one).collect();
    }

    //Column ranges: thread k takes columns [k*per, (k+1)*per)
    let per = n.div_ceil(threads);
    let ranges: Vec<std::ops::Range<usize>> = (0..threads)
        .map(|k| (k * per).min(n)..((k + 1) * per).min(n))
        .filter(|r| !r.is_empty())
        .collect();

    let chunks: Vec<Vec<Matrix6xX<f64>>> = std::thread::scope(|s| {
        //Spawn first, join after: all threads run at the same time, and `join` hands back each thread's Vec
        let handles: Vec<_> = ranges.iter()
            .map(|r| { let r = r.clone(); let one = &one; s.spawn(move || r.map(one).collect::<Vec<_>>()) })
            .collect();
        handles.into_iter().map(|hnd| hnd.join().expect("propagation thread panicked")).collect()
    });

    chunks.into_iter().flatten().collect()
}
