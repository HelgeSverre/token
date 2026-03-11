fn main() {
    let version = git_version();
    println!("cargo:rustc-env=TOKEN_VERSION={version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to set Windows icon: {}", e);
        }
    }
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
