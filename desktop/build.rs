#[cfg(target_os = "windows")]
fn main() {
    println!("cargo:rerun-if-changed=assets/lotus.ico");
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/lotus.ico");
    res.set("ProductName", "Lotus Download Manager");
    res.set("FileDescription", "Lotus Download Manager");
    res.set("LegalCopyright", "b0dr7");
    res.set("OriginalFilename", "LotusDM.exe");
    if let Err(err) = res.compile() {
        // The icon is cosmetic: never fail a build because rc.exe is missing.
        println!("cargo:warning=windows resources not compiled: {err}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
