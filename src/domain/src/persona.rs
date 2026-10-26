//! Fake device ("persona"): what sites read about the hardware. Common real-world values near
//! this computer's, never its own, picked per site and run (`scripts/privacy.js` does the
//! picking in the page; [`pick`] here is the same rule, kept in sync and tested).
//! Random values stand out; common ones put you in a crowd of millions.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Lists {
    /// `navigator.hardwareConcurrency`: common core counts near the real one, without it.
    pub cores: Vec<u32>,
    /// `navigator.deviceMemory` (Chromium only; Chrome caps it at 8).
    pub memory: Vec<u32>,
    /// WebGL unmasked (vendor, renderer) for this OS.
    pub gpus: Vec<(&'static str, &'static str)>,
    /// Common screen sizes; the page drops the real one and those smaller than the window.
    pub screens: Vec<(u32, u32)>,
    /// `userAgentData` `platformVersion` (Windows only; WebKit has no `userAgentData`).
    pub platform_versions: Vec<&'static str>,
}

const CORES: [u32; 8] = [4, 6, 8, 10, 12, 14, 16, 20];

/// The lists for `os` (`std::env::consts::OS`) and this machine's core count.
pub fn lists(os: &str, real_cores: u32) -> Lists {
    let near = |c: &&u32| **c != real_cores && **c * 2 >= real_cores && **c <= real_cores.max(4) * 2;
    let mut cores: Vec<u32> = CORES.iter().filter(near).copied().collect();
    if cores.is_empty() {
        cores = CORES.iter().filter(|c| **c != real_cores).copied().collect();
    }
    let (gpus, screens, platform_versions): (Vec<_>, Vec<_>, Vec<_>) = match os {
        // Safari reports the same generic GPU for every Mac: that's the crowd.
        "macos" => (vec![("Apple Inc.", "Apple GPU")], vec![(1440, 900), (1512, 982), (1470, 956), (1728, 1117), (1680, 1050), (1920, 1080), (2560, 1440), (3440, 1440), (3840, 2160)], vec![]),
        "windows" => (
            vec![
                ("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce GTX 1060 6GB Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (Intel)", "ANGLE (Intel, Intel(R) UHD Graphics 620 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (Intel)", "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (Intel)", "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (AMD)", "ANGLE (AMD, AMD Radeon(TM) Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (AMD)", "ANGLE (AMD, AMD Radeon RX 580 Series Direct3D11 vs_5_0 ps_5_0, D3D11)"),
                ("Google Inc. (AMD)", "ANGLE (AMD, AMD Radeon RX 6600 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
            ],
            vec![(1366, 768), (1536, 864), (1920, 1080), (1440, 900), (1600, 900), (2560, 1440), (1280, 720), (3440, 1440), (3840, 2160)],
            vec!["10.0.0", "13.0.0", "14.0.0", "15.0.0"],
        ),
        _ => (
            vec![
                ("Intel", "Mesa Intel(R) UHD Graphics 620 (KBL GT2)"),
                ("Intel", "Mesa Intel(R) UHD Graphics 630 (CFL GT2)"),
                ("Intel", "Mesa Intel(R) Xe Graphics (TGL GT2)"),
                ("AMD", "AMD Radeon Graphics (radeonsi, renoir, LLVM 17.0.6, DRM 3.54)"),
                ("AMD", "AMD Radeon RX 580 Series (radeonsi, polaris10, LLVM 17.0.6, DRM 3.54)"),
                ("NVIDIA Corporation", "NVIDIA GeForce GTX 1650/PCIe/SSE2"),
                ("NVIDIA Corporation", "NVIDIA GeForce RTX 3060/PCIe/SSE2"),
            ],
            vec![(1366, 768), (1920, 1080), (1600, 900), (2560, 1440), (1440, 900), (1280, 1024), (3440, 1440), (3840, 2160)],
            vec![],
        ),
    };
    Lists { cores, memory: vec![4, 8], gpus, screens, platform_versions }
}

/// Per-site seed: FNV-1a over the run seed and the host, as `privacy.js` computes it.
pub fn site_seed(run_seed: &str, host: &str) -> u32 {
    run_seed.encode_utf16().chain(host.encode_utf16()).fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619))
}

/// One option for this site, never `real`. Same seed and salt → same answer.
pub fn pick<T: Clone + PartialEq>(options: &[T], real: Option<&T>, seed: u32, salt: u32) -> Option<T> {
    let allowed: Vec<&T> = options.iter().filter(|o| Some(*o) != real).collect();
    let mut x = (seed ^ salt).wrapping_mul(2654435761);
    x ^= x >> 15;
    let x = x.wrapping_mul(2246822507);
    (!allowed.is_empty()).then(|| allowed[x as usize % allowed.len()].clone())
}
