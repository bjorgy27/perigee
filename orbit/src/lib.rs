/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// perigee-orbit: the orbit math shared by the Perigee engine (PC) and the Perigee viewer (desktop + TV).
///
/// Everything in here is plain arithmetic on nalgebra vectors: no network, no files, no clock. The callers
/// pass the time in as a Julian date. That is what lets the same code compile for the Fire TV.
///
///   constants  physical constants (mu, J2, Earth radius, rotation, drag density)
///   coe        classical orbital elements (a 9-row element set matrix) -> inertial state vectors
///   rk4        the Runge-Kutta 4 integrator with J2 + drag dynamics, one satellite at a time
///   parallel   propagate_all: every satellite at once, one thread per core
///   budget     Propagator: incremental propagation in slices of N milliseconds, for a live app
/////////////////////////////////////////////////////////////////////////////////////////////////////////
use nalgebra::{Const, Dyn, OMatrix};

pub mod constants;
pub mod coe;
pub mod rk4;
pub mod parallel;
pub mod budget;

//One element set per column, 9 rows:
//  0 NORAD ID          4 inclination (deg)        8 B* (1/earth radii)
//  1 epoch (JD)        5 RA of asc node (deg)
//  2 mean motion       6 arg of pericenter (deg)
//    (rev/day)         7 mean anomaly (deg)
//  3 eccentricity
pub type ElSetMatrix = OMatrix<f64, Const<9>, Dyn>;

pub use coe::{sv_from_coe, state_from_coe};
pub use rk4::{propagate, rk4_step};
pub use parallel::{propagate_all, propagate_all_threads};
pub use budget::Propagator;
