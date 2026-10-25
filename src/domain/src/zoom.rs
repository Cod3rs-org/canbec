//! Page zoom: fixed steps like other browsers, remembered per site.

pub const LEVELS: &[f64] = &[0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

/// Next zoom level: `1` bigger, `-1` smaller, `0` back to 100 %.
pub fn step(current: f64, dir: i8) -> f64 {
    let i = LEVELS.iter().position(|&l| l >= current - 0.001).unwrap_or(LEVELS.len() - 1);
    match dir {
        0 => 1.0,
        d if d > 0 => LEVELS[(i + 1).min(LEVELS.len() - 1)],
        _ => LEVELS[i.saturating_sub(1)],
    }
}
