use std::time::Instant;

use anyhow::Result;


mod search;






fn main()->Result<()>{
    let start = Instant::now();
    let index = search::build_index(r"C:\")?;
    println!(
        "Indexation : {:.2} secondes",
        start.elapsed().as_secs_f64()
    );
    
let start = Instant::now();

    let results = search::search_file(&index, "*codebook*")?;

     println!(
        "Recherche : {:.3} secondes",
        start.elapsed().as_secs_f64()
    );
    for path in results{
        println!("{}", path.display());
    }
    
    Ok(())
}
