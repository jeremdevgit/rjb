// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
// Génère l'icône .ico et l'intègre à l'exécutable avec le manifeste.

include!("src/icon.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out.join("rjb.ico");
    std::fs::write(&ico, ico_bytes(&[16, 20, 24, 32, 48, 64, 256])).unwrap();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap());
        res.set("ProductName", "rJb");
        res.set("FileDescription", "rJb - contrôle RGB unifié");
        res.set("CompanyName", "jeremdevgit");
        res.set("LegalCopyright", "Copyright (C) 2026 jeremdevgit - logiciel libre sous licence GPL-2.0-or-later");
        res.set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
<requestedExecutionLevel level="asInvoker" uiAccess="false"/>
</requestedPrivileges></security></trustInfo>
<application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
<dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
</windowsSettings></application>
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>"#,
        );
        res.compile().expect("ressources Windows");
        // les DLL importées sont cherchées dans System32 uniquement, jamais
        // dans le dossier de l'exécutable (installateur lancé depuis
        // Téléchargements, où une DLL piégée pourrait attendre)
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
}
