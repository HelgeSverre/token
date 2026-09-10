fn main() {
    compile_external_scanners();
    compile_fennel_grammar();
    compile_legacy_grammars();
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

fn compile_legacy_grammars() {
    for (name, include, files) in [
        (
            "tree-sitter-cue-compat",
            "vendor/tree-sitter-cue-compat",
            &[
                "vendor/tree-sitter-cue-compat/parser.c",
                "vendor/tree-sitter-cue-compat/scanner.c",
            ] as &[_],
        ),
        (
            "tree-sitter-pest-compat",
            "vendor/tree-sitter-pest-compat",
            &["vendor/tree-sitter-pest-compat/parser.c"] as &[_],
        ),
        (
            "tree-sitter-pony-compat",
            "vendor/tree-sitter-pony-compat",
            &[
                "vendor/tree-sitter-pony-compat/parser.c",
                "vendor/tree-sitter-pony-compat/scanner.c",
            ] as &[_],
        ),
    ] {
        let mut build = cc::Build::new();
        build.warnings(false);
        build.include(include);
        for file in files {
            build.file(file);
            println!("cargo:rerun-if-changed={file}");
        }
        build.compile(name);
    }
}

fn compile_fennel_grammar() {
    let parser = "vendor/tree-sitter-fennel-compat/parser.c";
    let scanner = "vendor/tree-sitter-fennel-compat/scanner.c";
    cc::Build::new()
        .warnings(false)
        .include("vendor/tree-sitter-fennel-compat")
        .file(parser)
        .file(scanner)
        .compile("tree-sitter-fennel-compat");
    println!("cargo:rerun-if-changed={parser}");
    println!("cargo:rerun-if-changed={scanner}");
}

fn compile_external_scanners() {
    for (name, scanner) in [
        (
            "tree-sitter-applescript-scanner",
            "vendor/tree-sitter-applescript-compat/scanner.c",
        ),
        (
            "tree-sitter-janet-scanner",
            "vendor/tree-sitter-janet-compat/scanner.c",
        ),
    ] {
        cc::Build::new()
            .warnings(false)
            // Both scanners use the retained standard Tree-sitter header.
            .include("vendor/tree-sitter-applescript-compat")
            .file(scanner)
            // Their callers live in dependency archives visited later by GNU
            // ld. Retain these two small objects regardless of archive order.
            .link_lib_modifier("+whole-archive")
            .compile(name);
        println!("cargo:rerun-if-changed={scanner}");
    }
}

fn compile_windows_resources() -> Result<(), Box<dyn std::error::Error>> {
    let version = env!("CARGO_PKG_VERSION");
    let numeric_version = windows_version(version);
    let icon_path = std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("token.ico");
    // ICO entries are limited to 256 pixels in either dimension.
    let icon = image::load_from_memory(include_bytes!("assets/icon.png"))?
        .resize(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
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
