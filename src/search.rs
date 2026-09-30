use std::{path::PathBuf, println, time::Instant};
use anyhow::{Ok, Result};
use walkdir::{DirEntry, WalkDir};
use globset::{GlobBuilder};
use std::collections::{BTreeMap, HashMap};

type FileIndex = HashMap<String, Vec<PathBuf>>;


fn should_visit(entry: &DirEntry) -> bool {
    if entry.file_type().is_file() {
        return true;
    }

    let name = entry.file_name().to_string_lossy().to_lowercase();

    !matches!(
        name.as_str(),
        "windows"
            | "program files"
            | "program files (x86)"
            | "programdata"
            | "$recycle.bin"
            | "system volume information"
            | "node_modules"
            | "target"
            | ".git"
            | "appdata"
            | "wsiaccount"
            | "perflogs"
            | "system repair"
            | "fichiers de conversation microsoft teams"
    )
}




pub fn build_index(folder:&str)-> Result<FileIndex>{
let mut index = FileIndex::new();


 for entry in WalkDir::new(folder)
 .follow_links(false)
    .into_iter()
    .filter_entry(should_visit)
    {
        let entry = match entry {
            std::result::Result::Ok(entry) => entry,
            Err(error) => {
                eprintln!("Impossible d'accéder à un élément : {error}");
                continue;
            }
        };

        if !entry.file_type().is_file(){
            continue;
        }

        let path = entry.into_path();

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        index
        .entry(file_name.to_ascii_lowercase())
        .or_default()
        .push(path);
    }
    Ok(index)
}



pub fn search_file(file:&FileIndex, pattern:&str) -> Result<Vec<PathBuf>>{
    let now = Instant::now();

    let glob = GlobBuilder::new(pattern)
        .case_insensitive(true)
        .literal_separator(true)
        .build()?
        .compile_matcher();

    let mut results = Vec::new();

    for (file_name, paths) in file {
        if glob.is_match(file_name){
            results.extend(paths.iter().cloned());
        }
    }

    
     println!(
        "{} résultat(s) trouvé(s) en {:.2} seconde",
        results.len(),
        now.elapsed().as_secs_f64()
    );

    Ok(results)

}