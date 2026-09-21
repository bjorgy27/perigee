/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Runga-Kutta 4 Integrator 
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
use nalgebra::{Vector3, Vector6, Matrix6xX};
use crate::constants::*;


//Integrate one satellite from t = 0 to t_final, storing the state after every step.
//Returns a 6 x M matrix: column i is the state at time i * h seconds (last column lands exactly on t_final).
pub fn propagate(y0: Vector6<f64>, t_final: f64, h: f64, bstar: f64) -> Matrix6xX<f64> {

    let mut y = y0;
    let mut t = 0.0;

    let mut states: Vec<Vector6<f64>> = Vec::new();
    states.push(y);

    while t < t_final {

        let step = if t + h > t_final { t_final - t } else { h };

        y = rk4_step(y, t, step, bstar);
        t += step;

        states.push(y);
    }

    Matrix6xX::from_columns(&states)
}



//One RK4 step of h seconds from state y at time t (seconds since epoch). Public so the budgeted
//Propagator can take single steps and keep its own state between frames.
pub fn rk4_step(y: Vector6<f64>, t: f64, h: f64, bstar: f64) -> Vector6<f64>{

    //K1 Term
    let k1 = dynamics(y,t,bstar);
    
    //K2 Term
    let k2 = dynamics(y + h * k1/2.0, t + (h/2.0), bstar);

    //K3 Term
    let k3 = dynamics(y + h * k2/2.0, t + (h/2.0), bstar);

    //K4 Term
    let k4 = dynamics(y + h * k3, t + h, bstar);

    y + (h / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4)



}








//Computing y_dot with CURRENT significant perturbations 
fn dynamics(y: Vector6<f64>, _t: f64, bstar: f64) -> Vector6<f64> {

    let r = Vector3::new (y[0], y[1], y[2]);
    let v = Vector3::new (y[3], y[4], y[5]);

    //Gravity Acceleration
    let a_g = -(MU / r.norm().powi(3)) * r;

    //J2 Perturbation
    let k = 1.5 * J2 * MU * RE.powi(2) / r.norm().powi(5);
    let zr = 5.0 * r[2].powi(2) / r.norm().powi(2);

    let a_j2_x = k * r[0] * (zr - 1.0);
    let a_j2_y = k * r[1] * (zr - 1.0);
    let a_j2_z = k * r[2] * (zr - 3.0);

    let a_j2 = Vector3::new(a_j2_x, a_j2_y, a_j2_z);

    //Atmosphereic Drag
    let v_rel = v - W_EARTH.cross(&r);
    let cd_a_m = 2.0 * bstar / RO_0;

    let a_drag = -0.5 * cd_a_m * RO * v_rel.norm() * v_rel / 1000.0;

    let a = a_g + a_j2 + a_drag;

    let y_dot = Vector6::new(v[0],v[1],v[2],a[0],a[1],a[2]);

    y_dot

}
