//! Miniaturen zoals Verkenner ze toont, via de Windows-shell (`IShellItemImageFactory`), als PNG
//! in de schijfcache `%LOCALAPPDATA%\Netjes\thumbs`. De UI laadt die PNG's en tekent zelf een
//! tegel voor bestanden zonder miniatuur. Niets hier mag panieken: elke fout wordt `None`.

use crate::paths::app_data_dir;
use image::ImageFormat;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

/// Maakt de namen van tijdelijke bestanden uniek, ook voor gelijktijdige aanroepen op één bestand.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Cachesleutel (blake3, hex) uit pad, grootte, wijzigingstijd en gevraagde pixelgrootte. Wijzigt
/// het bestand, dan hoort er een nieuwe miniatuur bij en blijft de oude gewoon ongebruikt staan.
pub fn thumb_cache_key(path: &Path, size: u64, modified_ms: u64, px: u32) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    // Vaste breedte achteraan het pad, dus er is geen dubbelzinnige overgang tussen de velden.
    hasher.update(&size.to_le_bytes());
    hasher.update(&modified_ms.to_le_bytes());
    hasher.update(&px.to_le_bytes());
    hasher.finalize().to_hex().to_string()
}

/// De miniatuur van één bestand in `app_data_dir()/thumbs`, of `None` als de shell er geen heeft.
pub fn get_thumbnail(path: &Path, px: u32) -> Option<PathBuf> {
    get_thumbnail_in(&default_cache_dir(), path, px)
}

/// Zoals [`get_thumbnail`], met een eigen cachemap (voor tests).
pub fn get_thumbnail_in(cache_dir: &Path, path: &Path, px: u32) -> Option<PathBuf> {
    let _com = shell::ComGuard::new();
    thumbnail_with_com(cache_dir, path, px)
}

/// Miniaturen voor een hele reeks paden, in dezelfde volgorde. COM wordt één keer voor de hele
/// reeks gestart. De aanroeper verdeelt in batches en draait dit op een blokkerende thread.
pub fn get_thumbnails(paths: &[String], px: u32) -> Vec<Option<String>> {
    get_thumbnails_in(&default_cache_dir(), paths, px)
}

/// Zoals [`get_thumbnails`], met een eigen cachemap (voor tests).
pub fn get_thumbnails_in(cache_dir: &Path, paths: &[String], px: u32) -> Vec<Option<String>> {
    let _com = shell::ComGuard::new();
    paths
        .iter()
        .map(|path| {
            thumbnail_with_com(cache_dir, Path::new(path), px)
                .map(|thumb| thumb.to_string_lossy().into_owned())
        })
        .collect()
}

fn default_cache_dir() -> PathBuf {
    app_data_dir().join("thumbs")
}

/// De kern; COM is op deze thread al gestart. Een treffer in de cache raakt de shell niet aan.
fn thumbnail_with_com(cache_dir: &Path, path: &Path, px: u32) -> Option<PathBuf> {
    // Enkel een bestaand gewoon bestand: een map of een weggevallen netwerkpad krijgt geen
    // shell-aanroep, want die kan blijven wachten.
    let metadata = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;
    let key = thumb_cache_key(path, metadata.len(), modified_ms(&metadata), px);
    let cached = cache_dir.join(format!("{key}.png"));
    if cached.is_file() {
        return Some(cached);
    }

    let image = shell::render(path, px)?;
    fs::create_dir_all(cache_dir).ok()?;
    // Eerst naar een tijdelijk bestand en dan hernoemen, zodat een lezer nooit een half PNG ziet.
    let tmp = cache_dir.join(format!(
        "{key}.{}.tmp",
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if image.save_with_format(&tmp, ImageFormat::Png).is_err() {
        let _ = fs::remove_file(&tmp);
        return None;
    }
    if fs::rename(&tmp, &cached).is_err() {
        let _ = fs::remove_file(&tmp);
        // Een gelijktijdige aanroep kan de miniatuur net zelf geplaatst hebben.
        return cached.is_file().then_some(cached);
    }
    Some(cached)
}

fn modified_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(windows)]
mod shell {
    use image::RgbaImage;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::path::{Path, PathBuf};
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
        GetDIBits, GetObjectW, HBITMAP, ReleaseDC,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY,
    };
    use windows::core::HSTRING;

    /// Houdt COM gestart zolang hij leeft en sluit het daarna weer af, maar enkel als wij het
    /// zelf startten.
    pub struct ComGuard {
        started: bool,
    }

    impl ComGuard {
        pub fn new() -> Self {
            // SAFETY: CoInitializeEx mag op elke thread, met een lege gereserveerde parameter.
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            // S_OK en S_FALSE (al gestart in dezelfde modus) vragen allebei om een CoUninitialize.
            // RPC_E_CHANGED_MODE betekent dat de thread al in een andere modus draait; COM is dan
            // bruikbaar maar niet van ons, en elke andere fout laat COM uit.
            Self {
                started: result.is_ok(),
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.started {
                // SAFETY: hoort bij de geslaagde CoInitializeEx van `new` op dezelfde thread.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Een GDI-bitmap die we van de shell kregen en altijd zelf vrijgeven, ook bij een fout.
    struct Bitmap(HBITMAP);

    impl Drop for Bitmap {
        fn drop(&mut self) {
            // SAFETY: de bitmap is van ons, nog niet vrijgegeven en in geen enkele DC geselecteerd.
            let _ = unsafe { DeleteObject(self.0.into()) };
        }
    }

    /// Vraagt de miniatuur aan de shell: enkel een echte miniatuur (geen pictogram), passend in
    /// `px` x `px` met behoud van de verhouding. COM moet op deze thread gestart zijn.
    pub fn render(path: &Path, px: u32) -> Option<RgbaImage> {
        let edge = i32::try_from(px).ok().filter(|&edge| edge > 0)?;
        // De shell weigert een `/` in het pad; `components` zet alle scheidingstekens om naar `\`.
        let wide = HSTRING::from(path.components().collect::<PathBuf>().as_os_str());
        // SAFETY: `wide` is een geldige, afgesloten UTF-16-tekst die de aanroep overleeft.
        let factory: IShellItemImageFactory =
            unsafe { SHCreateItemFromParsingName(&wide, None) }.ok()?;
        let flags = SIIGBF_THUMBNAILONLY | SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK;
        // SAFETY: `factory` is een levende COM-interface; de teruggegeven bitmap is voortaan van ons.
        let bitmap = Bitmap(unsafe { factory.GetImage(SIZE { cx: edge, cy: edge }, flags) }.ok()?);
        read_pixels(&bitmap)
    }

    fn read_pixels(bitmap: &Bitmap) -> Option<RgbaImage> {
        let mut info = BITMAP::default();
        // SAFETY: `info` is een BITMAP en de meegegeven grootte is precies die van een BITMAP.
        let written = unsafe {
            GetObjectW(
                bitmap.0.into(),
                size_of::<BITMAP>() as i32,
                Some((&mut info as *mut BITMAP).cast::<c_void>()),
            )
        };
        if written == 0 {
            return None;
        }
        let width = u32::try_from(info.bmWidth).ok().filter(|&w| w > 0)?;
        let height = u32::try_from(info.bmHeight).ok().filter(|&h| h > 0)?;

        // Negatieve hoogte: de rijen komen bovenaan beginnend (top-down) terug.
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: info.bmWidth,
                biHeight: -info.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        // SAFETY: GetDC(None) geeft de DC van het scherm of een ongeldige handle.
        let screen = unsafe { GetDC(None) };
        if screen.is_invalid() {
            return None;
        }
        // SAFETY: `pixels` is precies groot genoeg voor `height` rijen van 32 bit per pixel,
        // `header` beschrijft die indeling en de bitmap zit in geen enkele DC.
        let lines = unsafe {
            GetDIBits(
                screen,
                bitmap.0,
                0,
                height,
                Some(pixels.as_mut_ptr().cast::<c_void>()),
                &mut header,
                DIB_RGB_COLORS,
            )
        };
        // SAFETY: `screen` komt van GetDC(None) hierboven en wordt maar één keer vrijgegeven.
        unsafe { ReleaseDC(None, screen) };
        if lines != info.bmHeight {
            return None;
        }
        bgra_to_rgba(&mut pixels);
        RgbaImage::from_raw(width, height, pixels)
    }

    /// GDI levert BGRA; PNG wil RGBA. JPEG-miniaturen komen zonder alfakanaal terug (overal 0),
    /// en zo'n beeld zou onzichtbaar zijn: dan maken we alles ondoorzichtig.
    pub(super) fn bgra_to_rgba(pixels: &mut [u8]) {
        let (pixels, _) = pixels.as_chunks_mut::<4>();
        let transparent = pixels.iter().all(|px| px[3] == 0);
        for px in pixels {
            px.swap(0, 2);
            if transparent {
                px[3] = 255;
            }
        }
    }
}

/// Buiten Windows bestaat er geen shell: nooit een miniatuur.
#[cfg(not(windows))]
mod shell {
    use image::RgbaImage;
    use std::path::Path;

    pub struct ComGuard;

    impl ComGuard {
        pub fn new() -> Self {
            Self
        }
    }

    pub fn render(_path: &Path, _px: u32) -> Option<RgbaImage> {
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::fs;
    use tempfile::tempdir;

    /// Een effen 64x64 PNG; Windows heeft er via de ingebouwde beeldhandler een miniatuur voor.
    fn write_png(path: &Path) {
        RgbaImage::from_pixel(64, 64, Rgba([200, 40, 40, 255]))
            .save_with_format(path, ImageFormat::Png)
            .unwrap();
    }

    #[test]
    fn png_file_gets_cached_thumbnail() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let png = src.path().join("rood.png");
        write_png(&png);

        let first = get_thumbnail_in(cache.path(), &png, 96).expect("de shell heeft een miniatuur");
        assert!(first.is_file());
        assert!(first.starts_with(cache.path()));
        let mtime = fs::metadata(&first).unwrap().modified().unwrap();

        let second = get_thumbnail_in(cache.path(), &png, 96).expect("tweede aanroep uit de cache");
        assert_eq!(second, first);
        assert_eq!(fs::metadata(&second).unwrap().modified().unwrap(), mtime);
    }

    #[test]
    fn thumbnail_keeps_the_colours_of_the_picture() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let png = src.path().join("rood.png");
        write_png(&png);

        let thumb = get_thumbnail_in(cache.path(), &png, 96).unwrap();

        let decoded = image::open(&thumb).unwrap().to_rgba8();
        let centre = decoded.get_pixel(decoded.width() / 2, decoded.height() / 2);
        assert_eq!(*centre, Rgba([200, 40, 40, 255]));
    }

    #[test]
    fn forward_slashes_in_the_path_are_accepted() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let png = src.path().join("rood.png");
        write_png(&png);
        let slashed = PathBuf::from(png.to_string_lossy().replace('\\', "/"));

        assert!(get_thumbnail_in(cache.path(), &slashed, 96).is_some());
    }

    #[test]
    fn bgra_becomes_rgba_and_alpha_less_bitmaps_become_opaque() {
        let mut with_alpha = [10, 20, 30, 128, 1, 2, 3, 0];
        shell::bgra_to_rgba(&mut with_alpha);
        assert_eq!(with_alpha, [30, 20, 10, 128, 3, 2, 1, 0]);

        let mut without_alpha = [10, 20, 30, 0, 1, 2, 3, 0];
        shell::bgra_to_rgba(&mut without_alpha);
        assert_eq!(without_alpha, [30, 20, 10, 255, 3, 2, 1, 255]);
    }

    #[test]
    fn file_without_thumbnail_handler_returns_none() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let txt = src.path().join("notitie.txt");
        fs::write(&txt, "geen miniatuur hiervoor").unwrap();

        assert_eq!(get_thumbnail_in(cache.path(), &txt, 96), None);
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }

    #[test]
    fn cache_key_changes_with_size_and_mtime() {
        let p = Path::new(r"C:\Users\x\Downloads\foto.png");
        let base = thumb_cache_key(p, 1, 1, 96);
        assert_eq!(base, thumb_cache_key(p, 1, 1, 96));
        assert_ne!(base, thumb_cache_key(p, 2, 1, 96));
        assert_ne!(base, thumb_cache_key(p, 1, 2, 96));
        assert_ne!(base, thumb_cache_key(p, 1, 1, 128));
        let other = Path::new(r"C:\Users\x\Downloads\ander.png");
        assert_ne!(base, thumb_cache_key(other, 1, 1, 96));
    }

    #[test]
    fn missing_file_and_folder_return_none() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        assert_eq!(
            get_thumbnail_in(cache.path(), &src.path().join("weg.png"), 96),
            None
        );
        assert_eq!(get_thumbnail_in(cache.path(), src.path(), 96), None);
    }

    #[test]
    fn batch_keeps_order_and_maps_failures_to_none() {
        let src = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let a = src.path().join("a.png");
        let b = src.path().join("b.png");
        write_png(&a);
        write_png(&b);
        let paths: Vec<String> = [&a, &src.path().join("weg.png"), &b]
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        let result = get_thumbnails_in(cache.path(), &paths, 96);

        assert_eq!(result.len(), 3);
        assert!(result[0].as_deref().is_some_and(|p| Path::new(p).is_file()));
        assert_eq!(result[1], None);
        assert!(result[2].as_deref().is_some_and(|p| Path::new(p).is_file()));
        assert_ne!(result[0], result[2]);
    }
}
