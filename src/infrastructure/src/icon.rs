//! App icon: 7 built-in designs, or your own PNG.
//! Choose it in `<config>/icon`: a built-in name (`sprout`, `spoiler`, …) or a path to a PNG
//! (relative paths are resolved against the config folder). Anything invalid falls back to the default.

use std::io::Cursor;
use std::path::Path;

pub const DEFAULT_ICON: &str = "sprout";

/// (name, meaning, PNG bytes). Sources: docs/img/logo.svg and docs/img/logos/*.svg.
pub const BUILT_IN: &[(&str, &str, &[u8])] = &[
    ("sprout", "a human life growing back online", include_bytes!("icons/sprout.png")),
    ("strings", "cut puppet strings above a free human", include_bytes!("icons/strings.png")),
    ("fingerprint", "human uniqueness forming a C", include_bytes!("icons/fingerprint.png")),
    ("parrot", "browse as a human, not a parrot", include_bytes!("icons/parrot.png")),
    ("broken-link", "breaking the scroll habit", include_bytes!("icons/broken-link.png")),
    ("spoiler", "AI hidden, the human kept", include_bytes!("icons/spoiler.png")),
    ("calm-eye", "seeing the human web, protected", include_bytes!("icons/calm-eye.png")),
];

/// Largest custom icon we decode (keeps a huge file from eating memory).
const MAX_SIDE: u32 = 1024;

pub struct AppIcon {
    /// Built-in name or the custom file path, for messages.
    pub source: String,
    /// Original PNG (macOS Dock takes encoded image data).
    pub png: Vec<u8>,
    /// Decoded RGBA8 (window/taskbar icon on Windows and Linux).
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Reads `<config>/icon` and returns the chosen icon. Never fails: errors fall back to the default.
pub fn load_icon(config_dir: &Path) -> AppIcon {
    let choice = std::fs::read_to_string(config_dir.join("icon")).unwrap_or_default();
    let choice = choice.trim();
    if choice.is_empty() {
        return built_in(DEFAULT_ICON).expect("default icon is valid");
    }
    if let Some(icon) = built_in(choice) {
        return icon;
    }
    let path = config_dir.join(choice); // absolute paths replace config_dir
    let custom = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|png| decode(&png).map(|(rgba, width, height)| AppIcon { source: path.display().to_string(), png, rgba, width, height }));
    custom.unwrap_or_else(|e| {
        let names: Vec<_> = BUILT_IN.iter().map(|(n, _, _)| *n).collect();
        eprintln!("canbec: icon {choice:?} not usable ({e}); built-in names: {}", names.join(", "));
        built_in(DEFAULT_ICON).expect("default icon is valid")
    })
}

fn built_in(name: &str) -> Option<AppIcon> {
    let (name, _, png) = BUILT_IN.iter().find(|(n, _, _)| *n == name)?;
    let (rgba, width, height) = decode(png).ok()?;
    Some(AppIcon { source: (*name).into(), png: png.to_vec(), rgba, width, height })
}

/// PNG → RGBA8. Accepts RGBA, RGB, gray and gray+alpha (8-bit after expansion).
fn decode(png: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!("{width}x{height} is larger than {MAX_SIDE}x{MAX_SIDE}"));
    }
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let px = &buf[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => return Err(format!("unsupported PNG colour type {other:?}")),
    };
    Ok((rgba, width, height))
}
