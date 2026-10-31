use serde::{Deserialize, Serialize};

/// Categorie van een bestand. `Duplicaten` kent `categorize` nooit toe; dat beslist de duplicaatscan.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Fotos,
    Screenshots,
    Gifs,
    Design,
    Videos,
    Schermopnames,
    Pdfs,
    Documenten,
    Rekenbladen,
    Presentaties,
    Audio,
    Archieven,
    Installers,
    Code,
    Overig,
    Duplicaten,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Afbeeldingen,
    Videos,
    Documenten,
    Overige,
    Dubbel,
}

/// Beginnen met een van deze (kleine letters) en een videoformaat: schermopname.
const RECORDING_PREFIXES: &[&str] = &["schermopname", "screen recording", "opname"];

/// Beginnen met een van deze (kleine letters) en een foto-formaat: screenshot.
const SCREENSHOT_PREFIXES: &[&str] = &["schermafbeelding", "screenshot", "schermopname"];

/// Categorie op basis van de bestandsnaam. Naamregels winnen van de extensie, maar alleen
/// binnen het type dat bij de extensie hoort.
pub fn categorize(file_name: &str) -> Category {
    let name = file_name.trim().to_lowercase();
    let kind = by_extension(extension(&name));
    if kind == Category::Videos && starts_with_any(&name, RECORDING_PREFIXES) {
        return Category::Schermopnames;
    }
    if kind == Category::Fotos && starts_with_any(&name, SCREENSHOT_PREFIXES) {
        return Category::Screenshots;
    }
    kind
}

/// De tekst na de laatste punt. Een naam zonder punt heeft geen extensie.
fn extension(lower_name: &str) -> &str {
    lower_name.rsplit_once('.').map_or("", |(_, ext)| ext)
}

fn starts_with_any(name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| name.starts_with(prefix))
}

fn by_extension(ext: &str) -> Category {
    match ext {
        "jpg" | "jpeg" | "png" | "heic" | "heif" | "webp" | "tiff" | "bmp" | "raw" | "dng" => {
            Category::Fotos
        }
        "gif" => Category::Gifs,
        "fig" | "psd" | "ai" | "sketch" | "xd" | "afdesign" | "svg" => Category::Design,
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" => Category::Videos,
        "pdf" => Category::Pdfs,
        "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" => Category::Documenten,
        "xls" | "xlsx" | "csv" | "numbers" | "ods" => Category::Rekenbladen,
        "ppt" | "pptx" | "key" | "odp" => Category::Presentaties,
        "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" => Category::Audio,
        "zip" | "rar" | "7z" | "tar" | "gz" => Category::Archieven,
        "exe" | "msi" | "msix" | "dmg" | "pkg" | "appx" => Category::Installers,
        "js" | "ts" | "tsx" | "py" | "rs" | "json" | "html" | "css" | "sh" | "ps1" | "sql"
        | "yaml" | "yml" => Category::Code,
        _ => Category::Overig,
    }
}

/// Alle categorieën in de volgorde van de spec-tabel, `Duplicaten` laatst.
pub fn all() -> &'static [Category] {
    &[
        Category::Fotos,
        Category::Screenshots,
        Category::Gifs,
        Category::Design,
        Category::Videos,
        Category::Schermopnames,
        Category::Pdfs,
        Category::Documenten,
        Category::Rekenbladen,
        Category::Presentaties,
        Category::Audio,
        Category::Archieven,
        Category::Installers,
        Category::Code,
        Category::Overig,
        Category::Duplicaten,
    ]
}

/// De mapnaam zoals die op schijf komt.
pub fn folder_name(c: Category) -> &'static str {
    match c {
        Category::Fotos => "Foto's",
        Category::Screenshots => "Screenshots",
        Category::Gifs => "GIF's",
        Category::Design => "Design",
        Category::Videos => "Video's",
        Category::Schermopnames => "Schermopnames",
        Category::Pdfs => "PDF's",
        Category::Documenten => "Documenten",
        Category::Rekenbladen => "Rekenbladen",
        Category::Presentaties => "Presentaties",
        Category::Audio => "Audio",
        Category::Archieven => "Archieven",
        Category::Installers => "Installers",
        Category::Code => "Code",
        Category::Overig => "Overig",
        Category::Duplicaten => "Duplicaten",
    }
}

pub fn group(c: Category) -> Group {
    match c {
        Category::Fotos | Category::Screenshots | Category::Gifs | Category::Design => {
            Group::Afbeeldingen
        }
        Category::Videos | Category::Schermopnames => Group::Videos,
        Category::Pdfs | Category::Documenten | Category::Rekenbladen | Category::Presentaties => {
            Group::Documenten
        }
        Category::Audio
        | Category::Archieven
        | Category::Installers
        | Category::Code
        | Category::Overig => Group::Overige,
        Category::Duplicaten => Group::Dubbel,
    }
}

pub fn group_label(g: Group) -> &'static str {
    match g {
        Group::Afbeeldingen => "Afbeeldingen",
        Group::Videos => "Video's",
        Group::Documenten => "Documenten",
        Group::Overige => "Overige",
        Group::Dubbel => "Dubbel",
    }
}

#[derive(Serialize)]
pub struct CategoryInfo {
    pub id: Category,
    pub label: String,
    pub folder: String,
    pub group: Group,
    pub group_label: String,
    pub order: u32,
}

/// Eén `CategoryInfo` per categorie, in de volgorde van `all()`.
pub fn infos() -> Vec<CategoryInfo> {
    all()
        .iter()
        .enumerate()
        .map(|(index, &id)| {
            let folder = folder_name(id).to_string();
            let g = group(id);
            CategoryInfo {
                id,
                label: folder.clone(),
                folder,
                group: g,
                group_label: group_label(g).to_string(),
                order: index as u32,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_to_spec_table() {
        assert_eq!(categorize("vakantie.JPG"), Category::Fotos);
        assert_eq!(categorize("foto.heic"), Category::Fotos);
        assert_eq!(categorize("reactie.gif"), Category::Gifs);
        assert_eq!(categorize("ontwerp.fig"), Category::Design);
        assert_eq!(categorize("film.MOV"), Category::Videos);
        assert_eq!(categorize("factuur.pdf"), Category::Pdfs);
        assert_eq!(categorize("brief.docx"), Category::Documenten);
        assert_eq!(categorize("cijfers.csv"), Category::Rekenbladen);
        assert_eq!(categorize("deck.key"), Category::Presentaties);
        assert_eq!(categorize("opname.m4a"), Category::Audio);
        assert_eq!(categorize("archief.tar.gz"), Category::Archieven);
        assert_eq!(categorize("Setup.exe"), Category::Installers);
        assert_eq!(categorize("script.ps1"), Category::Code);
        assert_eq!(categorize("Makefile"), Category::Overig);
        assert_eq!(categorize("raar.xyz"), Category::Overig);
    }

    #[test]
    fn screenshot_and_recording_names_win_over_extension() {
        assert_eq!(
            categorize("Schermafbeelding 2026-10-31 123456.png"),
            Category::Screenshots
        );
        assert_eq!(categorize("Screenshot (12).png"), Category::Screenshots);
        assert_eq!(categorize("screenshot_final.jpg"), Category::Screenshots);
        assert_eq!(
            categorize("Schermopname 2026-10-31.png"),
            Category::Screenshots
        );
        assert_eq!(
            categorize("Schermopname 2026-10-31.mp4"),
            Category::Schermopnames
        );
        assert_eq!(
            categorize("Screen Recording 2026.mov"),
            Category::Schermopnames
        );
        assert_eq!(categorize("Opname 3.mp4"), Category::Schermopnames);
        assert_eq!(categorize("opname-3.mp3"), Category::Audio);
    }

    #[test]
    fn folder_names_and_groups_match_spec() {
        assert_eq!(folder_name(Category::Fotos), "Foto's");
        assert_eq!(folder_name(Category::Gifs), "GIF's");
        assert_eq!(group(Category::Code), Group::Overige);
        assert_eq!(group(Category::Duplicaten), Group::Dubbel);
        assert_eq!(all().len(), 16);
        assert_eq!(*all().last().unwrap(), Category::Duplicaten);
        assert_eq!(infos()[0].order, 0);
    }
}
