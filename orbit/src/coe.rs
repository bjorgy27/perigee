/////////////////////////////////////////////////////////////////////////////////////////////////////////
///
/// 
/// 
/// 
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
use nalgebra::{Vector3, Vector6, Matrix3, Matrix6xX};
use std::f64::consts::PI;
use crate::ElSetMatrix;

use crate::constants::*;

//Inertial state vector (km, km/s) of one column of the element set matrix, at that satellite's epoch.
//Mean anomaly -> eccentric anomaly (Newton) -> true anomaly -> perifocal r, v -> rotate by (w, i, RAAN).
pub fn state_from_coe(coe: &ElSetMatrix, col: usize) -> Vector6<f64> {

        let mm: f64      = coe[(2, col)];
        let e: f64       = coe[(3, col)];
        let incl: f64    = coe[(4, col)].to_radians();
        let raan: f64    = coe[(5, col)].to_radians();
        let w: f64       = coe[(6, col)].to_radians();
        let ma: f64      = coe[(7, col)].to_radians();

        let n: f64 = mm * 2.0 * PI / 86400.0;
        let a: f64 = (MU / (n * n)).cbrt();
        let h: f64 = (MU * a * (1.0 - (e * e))).sqrt();

        let mut ecc_anom: f64 = ma; 
            for _ in 0..50 {
                let f: f64 = ecc_anom - e * ecc_anom.sin() - ma;
                let fp: f64 = 1.0 - e * ecc_anom.cos();
                let d: f64 = f / fp;
                ecc_anom -= d;
                if d.abs() < 1e-12 { break; }
        }

        let theta = 2.0 * ((1.0 + e).sqrt() * (ecc_anom / 2.0).tan())
                    .atan2((1.0 - e).sqrt());

        let rp = (h * h / MU) / (1.0 + e * theta.cos()) * Vector3::new(theta.cos(), theta.sin(), 0.0);
        let vp = (MU / h) * Vector3::new(-(theta).sin(), e + theta.cos(), 0.0);

        let r3_w = Matrix3::new(raan.cos(), raan.sin(), 0.0,
                               -raan.sin(), raan.cos(), 0.0,
                                0.0,        0.0,        1.0);

        let r1_i = Matrix3::new(1.0,        0.0,        0.0,
                                0.0,        incl.cos(), incl.sin(),
                                0.0,       -incl.sin(), incl.cos());
        
        let r_3_w = Matrix3::new(w.cos(),    w.sin(),    0.0,
                               -w.sin(),    w.cos(),    0.0,
                                0.0,        0.0,        1.0);

        let q_p_x = (r_3_w * r1_i * r3_w).transpose();

        let r = q_p_x * rp;
        let v = q_p_x * vp;

        Vector6::new(r.x, r.y, r.z, v.x, v.y, v.z)
}

//Every column: initial state at its epoch, the epoch (JD), and how many seconds to propagate to reach t_end_jd.
//t_end_jd is passed in (the engine uses now + 0.5 day) so this crate never needs a clock.
pub fn sv_from_coe(coe: &ElSetMatrix, t_end_jd: f64) -> (Matrix6xX<f64>, Vec<f64>, Vec<f64>){

        let mut states: Vec<Vector6<f64>> = Vec::new();
        let mut jd_epochs: Vec<f64> = Vec::new();
        let mut dts: Vec<f64> = Vec::new();

    for col in 0..coe.ncols() {

        let x_0 = state_from_coe(coe, col);

        let jd_epoch: f64 = coe[(1, col)];
        let dt_seconds: f64 = (t_end_jd - jd_epoch) * 86400.0;

        states.push(x_0);
        jd_epochs.push(jd_epoch);
        dts.push(dt_seconds);
    }

        (Matrix6xX::from_columns(&states), jd_epochs, dts)

}
