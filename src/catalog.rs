/////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Intersect ELSET and NORADs to obtain only wanted ELSET cols
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
use crate::spacetrack::ElSetMatrix;


pub fn intersect(matrix: &ElSetMatrix, norads: &[u32]) -> ElSetMatrix { 

    let mut keep: Vec<usize> = Vec::new();

    for col in 0..matrix.ncols() {
        let id = matrix[(0, col)] as u32;

        if norads.contains(&id) {
            keep.push(col);
        }
    }

 matrix.select_columns(&keep)

}