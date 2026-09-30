use std::{path::PathBuf, println, time::Instant};
use anyhow::Result;
use globset::{GlobBuilder};
use std::collections::{HashMap};

type FileIndex = HashMap<String, Vec<PathBuf>>;


fn should_visit(entry: &DirEntry) -> bool {
    if entry.file_type().expect("REASON").is_file() {
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




use ignore::{DirEntry, WalkBuilder, WalkState};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

pub fn build_index(folder: &str) -> Result<FileIndex> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let errors = AtomicUsize::new(0);

    WalkBuilder::new(folder)
        .standard_filters(false) // désactive .gitignore, fichiers cachés, etc.
        .follow_links(false)
        .filter_entry(should_visit)
        .build_parallel()
        .run(|| {
            let tx = tx.clone();
            let errors = &errors;
            Box::new(move |entry| {
                match entry {
                    Ok(entry) if entry.file_type().is_some_and(|t| t.is_file()) => {
                        let _ = tx.send(entry.into_path());
                    }
                    Ok(_) => {}
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
                WalkState::Continue
            })
        });

    drop(tx); // ferme le canal pour que la boucle ci-dessous se termine

    let mut index = FileIndex::new();
    for path in rx {
        if let Some(name) = path.file_name() {
            let key = name.to_string_lossy().to_lowercase();
            index.entry(key).or_default().push(path);
        }
    }

    eprintln!("{} élément(s) inaccessible(s)", errors.load(Ordering::Relaxed));
    Ok(index)
}



pub fn search_file(file:&FileIndex, pattern:&str) -> Result<Vec<PathBuf>>{
    let now = Instant::now();
    let is_glob = pattern
        .chars()
        .any(|c| matches!(c, '*' | '?' | '[' | '{'));

    // Nom exact : une seule recherche dans la table, pas de parcours
    if !is_glob {
        let mut results = file
            .get(&pattern.to_lowercase())
            .cloned()
            .unwrap_or_default();
        results.sort();
        return Ok(results);
    }

    let glob = GlobBuilder::new(pattern)
        .case_insensitive(true)
        .literal_separator(true)
        .build()?
        .compile_matcher();

    let mut results: Vec<PathBuf> = file
    .iter()
    .filter(|(name, _)| glob.is_match(name.as_str()))
    .flat_map(|(_, paths)| paths.iter().cloned())
    .collect();

    results.sort();
     println!(
        "{} résultat(s) trouvé(s) en {:.2} seconde",
        results.len(),
        now.elapsed().as_secs_f64()
    );

    Ok(results)

}