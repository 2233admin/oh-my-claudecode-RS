#[cfg(windows)]
const OUT_DIR: &str = "OUT_DIR";
#[cfg(windows)]
const TARGET: &str = "TARGET";

fn main() {
    // Embed a Windows manifest requesting asInvoker to prevent UAC elevation
    // when the binary name contains "installer" (Windows heuristic).
    #[cfg(windows)]
    {
        let out_dir = std::env::var(OUT_DIR).unwrap();

        let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;
        let manifest_path = std::path::Path::new(&out_dir).join("installer.manifest");
        std::fs::write(&manifest_path, manifest).unwrap();

        let rc_content = "#include <winuser.h>\n1 RT_MANIFEST \"installer.manifest\"\n";
        let rc_path = std::path::Path::new(&out_dir).join("installer.rc");
        std::fs::write(&rc_path, rc_content).unwrap();

        let target = std::env::var(TARGET).unwrap();
        if target.contains("windows") {
            let windres = if target.contains("gnu") {
                "windres"
            } else {
                "rc"
            };
            let obj_path = std::path::Path::new(&out_dir).join("installer_manifest.o");
            let status = std::process::Command::new(windres)
                .arg("-i")
                .arg(&rc_path)
                .arg("-o")
                .arg(&obj_path)
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("cargo:rustc-link-arg={}", obj_path.display());
                }
                _ => {
                    // windres not available; skip manifest embedding
                    println!("cargo:warning=windres not found, skipping manifest embedding");
                }
            }
        }
    }
}
