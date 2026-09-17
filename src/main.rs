mod network;
use network::get_sat_data;

mod orbitprop;
use orbitprop::parse_mean_elements;

fn main() ->  Result<(), Box<dyn std::error::Error>> {
    
    //Logging into space-track and requesting 
    get_sat_data()?;

    let xml = std::fs::read_to_string("ELSET.xml")?;
    parse_mean_elements(&xml)?;

    Ok(())

} 

   

