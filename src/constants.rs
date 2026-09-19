use nalgebra::Vector3;

pub const MU: f64 = 398600.4418;
pub const RE: f64 = 6378.0;
pub const J2: f64 = 1.08263e-3;
pub const W_EARTH: Vector3<f64> = Vector3::new(0.0, 0.0, 7.2921159e-5);
pub const RO_0: f64 = 2.461e-5;
pub const RO: f64 = 3.7e-12; //Rough density until height model is implemented

