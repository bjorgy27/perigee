use roxmltree::Document;
use nalgebra::{vector,Vector6};


pub fn parse_mean_elements(xml: &str) -> Result<Vector6<f64>, Box<dyn std::error::Error>> {
    let doc = Document::parse(xml)?;

    
    let mean_elements_node = doc
        .descendants()
        .find(|n| n.has_tag_name("meanElements"))
        .ok_or("meanElements node not found")?;

    let get = |tag: &str| -> Result<String, Box<dyn std::error::Error>> {
        mean_elements_node
            .children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(str::to_string)
            .ok_or_else(|| format!("{tag} not found").into())
    };

    let mean_elements = vector![
            get("MEAN_MOTION")?.parse()?,
            get("ECCENTRICITY")?.parse()?,
            get("INCLINATION")?.parse()?,
            get("RA_OF_ASC_NODE")?.parse()?,
            get("ARG_OF_PERICENTER")?.parse()?,
            get("MEAN_ANOMALY")?.parse()?,
            
    ];

    println!("Vector: {:?}", mean_elements);

     Ok(mean_elements)
}
