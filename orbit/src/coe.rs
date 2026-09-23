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
//
//The element set holds TLE MEAN elements, not osculating ones. Taking the mean motion as if it were
//osculating puts the semi-major axis about 6 km low for a LEO satellite; integrated, that is a period
//about 8 s per orbit too short and the track runs ahead of the real satellite by ~5 s per hour since
//the element epoch (measured against SGP4 and a live tracker, 2026-09-23: 104 s after 20 h). Two
//corrections fix that, both first order in J2:
//  1. Kozai -> Brouwer: the TLE mean motion n0 is Kozai's; the SGP4 initialisation formulas
//     (a1, delta1, a0, delta0) recover Brouwer's mean semi-major axis a''.
//  2. Short-period J2 term: the osculating semi-major axis at the epoch is a'' plus
//        (3/2) J2 RE^2 / a'' * [ ((a''/r)^3 - 1/eta^3) (1 - 3/2 sin^2 i) + (a''/r)^3 sin^2 i cos 2u ]
//     with eta = sqrt(1 - e^2) and u = w + true anomaly. Against SGP4's osculating state at the epoch
//     this is right to 0.01 km rms over the whole catalog (0.3 km for e between 0.01 and 0.1).
//The other short-period terms (radius, angles) are left out: ~12 km of position, not growing.
pub fn state_from_coe(coe: &ElSetMatrix, col: usize) -> Vector6<f64> {

        let mm: f64      = coe[(2, col)];
        let e: f64       = coe[(3, col)];
        let incl: f64    = coe[(4, col)].to_radians();
        let raan: f64    = coe[(5, col)].to_radians();
        let w: f64       = coe[(6, col)].to_radians();
        let ma: f64      = coe[(7, col)].to_radians();

        let n: f64 = mm * 2.0 * PI / 86400.0;

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

        let a: f64 = osculating_a(n, e, incl, w, ecc_anom, theta);
        let h: f64 = (MU * a * (1.0 - (e * e))).sqrt();

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

//Osculating semi-major axis at the epoch from the TLE (Kozai) mean motion n0 (rad/s), see above.
pub fn osculating_a(n0: f64, e: f64, incl: f64, w: f64, ecc_anom: f64, theta: f64) -> f64 {
        let k2: f64 = J2 * RE * RE / 2.0;
        let cos_i: f64 = incl.cos();
        let sin2_i: f64 = incl.sin().powi(2);
        let eta3: f64 = (1.0 - e * e).powf(1.5);

        //1. Kozai mean motion -> Brouwer mean semi-major axis (SGP4 initialisation)
        let a1: f64 = (MU / (n0 * n0)).cbrt();
        let d1: f64 = 1.5 * k2 / (a1 * a1) * (3.0 * cos_i * cos_i - 1.0) / eta3;
        let a0: f64 = a1 * (1.0 - d1 / 3.0 - d1 * d1 - 134.0 / 81.0 * d1 * d1 * d1);
        let d0: f64 = 1.5 * k2 / (a0 * a0) * (3.0 * cos_i * cos_i - 1.0) / eta3;
        let a_mean: f64 = a0 / (1.0 - d0);

        //2. First-order J2 short-period term in the semi-major axis
        let a_over_r3: f64 = (1.0 / (1.0 - e * ecc_anom.cos())).powi(3);   // (a/r)^3 = 1/(1 - e cos E)^3
        let u: f64 = w + theta;
        let da: f64 = 1.5 * J2 * RE * RE / a_mean
                    * ((a_over_r3 - 1.0 / eta3) * (1.0 - 1.5 * sin2_i) + a_over_r3 * sin2_i * (2.0 * u).cos());
        a_mean + da
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rk4::propagate;

    //ISS element set of 2026-09-22 06:30:37 UTC (Space-Track). SGP4's osculating state at that epoch has
    //a = 6803.62 km; the mean motion taken as-is gives 6797.13 km. The old track ran 104 s ahead of a live
    //tracker after 20.5 h; started from the corrected state the same RK4 stays within 16 km of SGP4.
    fn iss() -> ElSetMatrix {
        ElSetMatrix::from_column_slice(&[25544.0, 2461305.77126732, 15.49224498, 0.00047657, 51.6312, 179.6046, 167.6102, 192.5004, 0.0001364276])
    }

    fn osc_a(y: &Vector6<f64>) -> f64 {
        let r = Vector3::new(y[0], y[1], y[2]).norm();
        let v2 = y[3] * y[3] + y[4] * y[4] + y[5] * y[5];
        1.0 / (2.0 / r - v2 / MU)
    }

    #[test]
    fn iss_starts_on_the_osculating_orbit() {
        let y = state_from_coe(&iss(), 0);
        let a = osc_a(&y);
        assert!((a - 6803.62).abs() < 0.1, "a = {a}");
        //Radius still carries its own (uncorrected) short-period term: a few km at the epoch, not growing
        let r = Vector3::new(y[0], y[1], y[2]).norm();
        assert!((r - 6801.03).abs() < 8.0, "r = {r}");
    }

    #[test]
    fn iss_track_stays_with_sgp4() {
        //SGP4 (python-sgp4 2.27, WGS72) positions from the same element set, km, TEME
        let refs = [(0.0, [-6800.871, 46.937, -0.004]), (6.0, [-4748.683, 3108.942, -3751.664]),
                    (12.0, [138.37, 4222.34, -5331.478]), (20.5, [-683.532, -4169.334, 5314.945])];
        let m = propagate(state_from_coe(&iss(), 0), 20.5 * 3600.0, 60.0, 0.0001364276);
        for (hh, r) in refs {
            let c = (hh * 60.0) as usize;
            let miss = (Vector3::new(m[(0, c)], m[(1, c)], m[(2, c)]) - Vector3::new(r[0], r[1], r[2])).norm();
            assert!(miss < 20.0, "{hh} h: {miss:.1} km from SGP4 (was 796 km at 20.5 h before the correction)");
        }
    }

    #[test]
    fn short_period_term_swings_with_the_argument_of_latitude() {
        //e = 0: a_osc = a'' + (3/2) J2 RE^2 / a'' * sin^2 i * cos 2u. Between u = 0 and u = 90 deg the
        //difference is twice that amplitude, whatever the Brouwer part is.
        let n0 = 15.0 * 2.0 * PI / 86400.0;
        let i = 60f64.to_radians();
        let a_u0 = osculating_a(n0, 0.0, i, 0.0, 0.0, 0.0);
        let a_u90 = osculating_a(n0, 0.0, i, PI / 2.0, 0.0, 0.0);
        let amp = 1.5 * J2 * RE * RE / a_u0 * i.sin().powi(2);
        assert!((a_u0 - a_u90 - 2.0 * amp).abs() < 0.05, "swing {} vs {}", a_u0 - a_u90, 2.0 * amp);   // amp uses a_osc for a', 1e-3 relative
    }
}
