use crate::categories::Category;
use crate::scan::FileEntry;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const BUFFER_SIZE: usize = 1024 * 1024;

/// Markeert dubbele bestanden en geeft het aantal gemarkeerde terug. Groepeert op grootte en
/// hasht enkel groepen met minstens twee niet-plaatshouders; lege bestanden tellen niet mee.
/// Per gelijke hash blijft het oudste bestand origineel, de rest krijgt `is_duplicate`,
/// `duplicate_of` en `Category::Duplicaten`.
pub fn mark_duplicates(files: &mut [FileEntry]) -> u32 {
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, file) in files.iter().enumerate() {
        if file.placeholder || file.size == 0 {
            continue;
        }
        by_size.entry(file.size).or_default().push(index);
    }

    let mut marked = 0;
    for indices in by_size.values().filter(|indices| indices.len() >= 2) {
        let mut by_hash: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
        for &index in indices {
            // Een bestand dat niet leesbaar is, is geen duplicaat en stopt de rest niet.
            if let Ok(hash) = hash_file(Path::new(&files[index].path)) {
                by_hash.entry(hash).or_default().push(index);
            }
        }

        for group in by_hash.values().filter(|group| group.len() >= 2) {
            let Some(&original) = group
                .iter()
                .min_by_key(|&&index| (files[index].modified_ms, files[index].id))
            else {
                continue;
            };
            let original_id = files[original].id;
            for &index in group.iter().filter(|&&index| index != original) {
                let file = &mut files[index];
                file.is_duplicate = true;
                file.duplicate_of = Some(original_id);
                file.category = Category::Duplicaten;
                marked += 1;
            }
        }
    }
    marked
}

/// Blake3-hash van de inhoud, gestreamd met een buffer van 1 MiB.
pub fn hash_file(p: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(p)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                hasher.update(&buffer[..read]);
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn entry(id: u32, path: &Path, size: u64, modified_ms: u64, placeholder: bool) -> FileEntry {
        FileEntry {
            id,
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            size,
            modified_ms,
            category: Category::Fotos,
            is_duplicate: false,
            duplicate_of: None,
            placeholder,
        }
    }

    #[test]
    fn identical_content_marks_newest_as_duplicate() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        let b = dir.path().join("a (1).jpg");
        fs::write(&a, b"zelfde inhoud").unwrap();
        fs::write(&b, b"zelfde inhoud").unwrap();
        let mut files = vec![
            entry(0, &a, 13, 1_000, false),
            entry(1, &b, 13, 11_000, false),
        ];

        assert_eq!(mark_duplicates(&mut files), 1);
        assert!(!files[0].is_duplicate);
        assert!(files[1].is_duplicate);
        assert_eq!(files[1].duplicate_of, Some(0));
        assert_eq!(files[1].category, Category::Duplicaten);
    }

    #[test]
    fn same_size_different_content_is_not_duplicate() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.bin");
        let b = dir.path().join("b.bin");
        fs::write(&a, [b'a'; 100]).unwrap();
        fs::write(&b, [b'b'; 100]).unwrap();
        let mut files = vec![
            entry(0, &a, 100, 1_000, false),
            entry(1, &b, 100, 2_000, false),
        ];

        assert_eq!(mark_duplicates(&mut files), 0);
        assert!(
            files
                .iter()
                .all(|f| !f.is_duplicate && f.duplicate_of.is_none())
        );
    }

    #[test]
    fn placeholder_is_never_hashed() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("a (1).pdf");
        fs::write(&a, b"zelfde inhoud").unwrap();
        fs::write(&b, b"zelfde inhoud").unwrap();
        let mut files = vec![entry(0, &a, 13, 1_000, true), entry(1, &b, 13, 2_000, true)];

        assert_eq!(mark_duplicates(&mut files), 0);
        assert!(files.iter().all(|f| !f.is_duplicate));
    }

    #[test]
    fn placeholder_is_neither_original_nor_duplicate() {
        let dir = tempdir().unwrap();
        let cloud = dir.path().join("cloud.pdf");
        let local = dir.path().join("local.pdf");
        fs::write(&cloud, b"zelfde inhoud").unwrap();
        fs::write(&local, b"zelfde inhoud").unwrap();
        let mut files = vec![
            entry(0, &cloud, 13, 1_000, true),
            entry(1, &local, 13, 2_000, false),
        ];

        assert_eq!(mark_duplicates(&mut files), 0);
        assert!(files.iter().all(|f| !f.is_duplicate));
    }

    #[test]
    fn oldest_stays_original_with_smallest_id_as_tiebreak() {
        let dir = tempdir().unwrap();
        let paths: Vec<_> = ["x.png", "y.png", "z.png"]
            .iter()
            .map(|n| dir.path().join(n))
            .collect();
        for p in &paths {
            fs::write(p, b"zelfde inhoud").unwrap();
        }
        let mut files = vec![
            entry(10, &paths[0], 13, 3_000, false),
            entry(20, &paths[1], 13, 1_000, false),
            entry(30, &paths[2], 13, 1_000, false),
        ];

        assert_eq!(mark_duplicates(&mut files), 2);
        assert_eq!(
            files.iter().map(|f| f.id).collect::<Vec<_>>(),
            vec![10, 20, 30]
        );
        assert!(!files[1].is_duplicate);
        assert_eq!(files[0].duplicate_of, Some(20));
        assert_eq!(files[2].duplicate_of, Some(20));
        assert!(files[0].is_duplicate && files[2].is_duplicate);
    }

    #[test]
    fn zero_size_files_are_never_duplicates() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"").unwrap();
        fs::write(&b, b"").unwrap();
        let mut files = vec![entry(0, &a, 0, 1_000, false), entry(1, &b, 0, 2_000, false)];

        assert_eq!(mark_duplicates(&mut files), 0);
        assert!(files.iter().all(|f| !f.is_duplicate));
    }

    #[test]
    fn unreadable_file_is_skipped_and_others_are_still_marked() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        let missing = dir.path().join("verdwenen.txt");
        fs::write(&a, b"zelfde inhoud").unwrap();
        fs::write(&b, b"zelfde inhoud").unwrap();
        let mut files = vec![
            entry(0, &a, 13, 1_000, false),
            entry(1, &missing, 13, 500, false),
            entry(2, &b, 13, 2_000, false),
        ];

        assert_eq!(mark_duplicates(&mut files), 1);
        assert!(!files[1].is_duplicate);
        assert!(files[2].is_duplicate);
        assert_eq!(files[2].duplicate_of, Some(0));
    }

    #[test]
    fn hash_file_streams_large_files_and_matches_blake3() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("groot.bin");
        let data: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i % 251) as u8).collect();
        fs::write(&p, &data).unwrap();

        assert_eq!(hash_file(&p).unwrap(), *blake3::hash(&data).as_bytes());
    }
}
