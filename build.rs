fn main() {
    let version = git_version();
    println!("cargo:rustc-env=TOKEN_VERSION={version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Err(error) = compile_windows_resources() {
            println!("cargo:warning=Failed to compile Windows resources: {error}");
        }
    }
}

fn compile_windows_resources() -> Result<(), Box<dyn std::error::Error>> {
    let version = env!("CARGO_PKG_VERSION");
    let numeric_version = windows_version(version);
    let icon_path = std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("token.ico");
    let icon = image::load_from_memory(include_bytes!("assets/icon.png"))?.to_rgba8();
    image::DynamicImage::ImageRgba8(icon).save_with_format(&icon_path, image::ImageFormat::Ico)?;

    let mut res = winres::WindowsResource::new();
    res.set_icon(icon_path.to_string_lossy().as_ref())
        .set("FileDescription", env!("CARGO_PKG_DESCRIPTION"))
        .set("FileVersion", version)
        .set("ProductVersion", version)
        .set("ProductName", "Token")
        .set("InternalName", "token")
        .set("OriginalFilename", "token.exe")
        .set("CompanyName", "Helge Sverre")
        .set("Comments", env!("CARGO_PKG_HOMEPAGE"))
        .set("LegalCopyright", "Copyright (c) Helge Sverre 2026")
        .set_version_info(winres::VersionInfo::FILEVERSION, numeric_version)
        .set_version_info(winres::VersionInfo::PRODUCTVERSION, numeric_version);

    res.compile()?;
    Ok(())
}

fn windows_version(version: &str) -> u64 {
    version
        .split('.')
        .take(4)
        .map(|part| {
            part.split_once('-')
                .map_or(part, |(numeric, _)| numeric)
                .parse::<u16>()
                .unwrap_or(0) as u64
        })
        .chain(std::iter::repeat(0))
        .take(4)
        .fold(0, |packed, part| (packed << 16) | part)
}

fn git_version() -> String {
    let cargo_version = env!("CARGO_PKG_VERSION");

    let output = std::process::Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let describe = String::from_utf8_lossy(&out.stdout).trim().to_string();
            describe.strip_prefix('v').unwrap_or(&describe).to_string()
        }
        _ => cargo_version.to_string(),
    }
}
