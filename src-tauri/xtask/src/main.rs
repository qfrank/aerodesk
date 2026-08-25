fn main() {
    check_system_deps();
    let args: Vec<String> = std::env::args().skip(1).collect();
    tauri_cli::run(args, Some("cargo tauri".to_string()));
}

#[cfg(target_os = "linux")]
fn check_system_deps() {
    use std::process::Command;

    let mut missing: Vec<&str> = Vec::new();

    if !Command::new("pkg-config")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        missing.push("pkg-config");
    }

    let libs: [(&str, &str); 6] = [
        ("dbus-1", "libdbus-1-dev"),
        ("webkit2gtk-4.1", "libwebkit2gtk-4.1-dev"),
        ("openssl", "libssl-dev"),
        ("ayatana-appindicator3-0.1", "libayatana-appindicator3-dev"),
        ("librsvg-2.0", "librsvg2-dev"),
        ("atspi-2", "at-spi2-core"),
    ];

    for (pkg, apt) in libs {
        let ok = Command::new("pkg-config")
            .args(["--exists", pkg])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            missing.push(apt);
        }
    }

    if !std::path::Path::new("/usr/include/xdo.h").exists() {
        missing.push("libxdo-dev");
    }

    if !Command::new("cc")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        missing.push("build-essential");
    }

    let a11y = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "toolkit-accessibility"])
        .output();
    if let Ok(out) = a11y {
        let val = String::from_utf8_lossy(&out.stdout);
        if val.trim() == "false" {
            let _ = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.desktop.interface",
                    "toolkit-accessibility",
                    "true",
                ])
                .status();
            println!("Enabled GNOME accessibility (needed for AT-SPI text injection).");
        }
    }

    if missing.is_empty() {
        return;
    }

    println!("Missing Linux system dependencies: {}", missing.join(", "));
    println!("Installing via apt (sudo password may be required)...");
    let status = Command::new("sudo")
        .arg("apt-get")
        .args(["install", "-y"])
        .args(&missing)
        .status();
    match status {
        Ok(s) if s.success() => println!("System dependencies installed."),
        _ => {
            eprintln!("Failed to install system dependencies. Run manually:");
            eprintln!("  sudo apt install {}", missing.join(" "));
            std::process::exit(1);
        }
    }
}

#[cfg(target_os = "macos")]
fn check_system_deps() {
    use std::process::Command;
    let ok = Command::new("xcode-select")
        .arg("-p")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        println!("Xcode Command Line Tools not found. Run: xcode-select --install");
    }
}

#[cfg(target_os = "windows")]
fn check_system_deps() {
    let vs = std::path::Path::new(r"C:\Program Files\Microsoft Visual Studio");
    let vs86 = std::path::Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio");
    if !vs.exists() && !vs86.exists() {
        println!("Visual Studio Build Tools not found. Install the \"Desktop development with C++\" workload from https://visualstudio.microsoft.com/");
    }
}
