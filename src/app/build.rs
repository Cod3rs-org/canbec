//! Embeds the app icon into canbec.exe on Windows (Explorer and pinned taskbar icons come from
//! the .exe, not from the window icon set at runtime). Other OSes: nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=canbec.rc");
    println!("cargo:rerun-if-changed=icons/canbec.ico");
    #[cfg(windows)]
    embed_resource::compile("canbec.rc", embed_resource::NONE).manifest_optional().expect("embed the Windows icon");
}
