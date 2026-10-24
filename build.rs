//! Embeds the icon/manifest and refuses to build an artifact whose displayed
//! version disagrees with Cargo.toml. `ProductVersion` in Explorer, the
//! manifest and the footer must never drift apart.

fn main() {
    for path in [
        "icons/icon.rc",
        "icons/icon.ico",
        "icons/app.manifest",
        "Cargo.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let parts: Vec<u64> = version
        .split('.')
        .map(|part| {
            part.parse()
                .unwrap_or_else(|_| panic!("GLYPH version must be numeric x.y.z, got {version}"))
        })
        .collect();
    assert_eq!(parts.len(), 3, "GLYPH version must be x.y.z, got {version}");
    let file_version = format!("{},{},{},0", parts[0], parts[1], parts[2]);

    let rc = std::fs::read_to_string("icons/icon.rc").expect("read icons/icon.rc");
    for expected in [
        format!("FILEVERSION {file_version}"),
        format!("PRODUCTVERSION {file_version}"),
        format!("\"ProductVersion\", \"{version}\\0\""),
        format!("\"FileVersion\", \"{version}\\0\""),
    ] {
        assert!(
            rc.contains(&expected),
            "icons/icon.rc is out of sync with Cargo.toml ({version}): missing {expected}"
        );
    }

    let manifest = std::fs::read_to_string("icons/app.manifest").expect("read icons/app.manifest");
    let identity = format!("version=\"{version}.0\"");
    assert!(
        manifest.contains(&identity),
        "icons/app.manifest is out of sync with Cargo.toml: missing {identity}"
    );

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("icons/icon.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
