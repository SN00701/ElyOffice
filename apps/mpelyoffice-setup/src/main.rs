//! The installer: one program that puts MPelyoffice on this computer.
//!
//! It installs for the current user by default, so no administrator rights
//! are needed. The file types are offered in "Open with"; the default
//! program of the system is never taken (docs/mp/05 §3.2).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

/// The scripts the installer runs; they live with the other Windows scripts.
const REGISTER: &str = include_str!("../../../packaging/windows/register-types.ps1");

/// The name of the program and of its uninstall entry.
const APP: &str = "MPelyoffice";
const EXE: &str = "MPelyoffice.exe";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\MPelyoffice";

/// What the installer was asked to do.
struct Options {
    /// Where the program goes.
    dir: PathBuf,
    /// The folder that holds the program to install.
    payload: PathBuf,
    /// Say nothing and ask nothing.
    silent: bool,
    /// Leave the file types alone.
    no_associations: bool,
    /// Take the program away again.
    uninstall: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("MPelyoffice 安装失败 / the install failed: {error:#}");
        std::process::exit(1);
    }
}

/// The program's own folder, next to its name.
fn default_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Programs").join(APP)
}

/// The folder the program was installed into, as the uninstall entry says.
fn installed_dir() -> Option<PathBuf> {
    let output = Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\MPelyoffice", "/v", "InstallLocation"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text.split('\r').find_map(|line| line.strip_prefix("    InstallLocation    REG_SZ    "))?;
    Some(PathBuf::from(value.trim()))
}

fn run() -> anyhow::Result<()> {
    let mut options = Options {
        dir: default_dir(),
        payload: std::env::current_exe()
            .context("the installer's own place")?
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default(),
        silent: false,
        no_associations: false,
        uninstall: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "/silent" | "/S" | "--silent" => options.silent = true,
            "/uninstall" | "--uninstall" => options.uninstall = true,
            "/noassoc" | "--no-associations" => options.no_associations = true,
            "/dir" | "--dir" => {
                let value = args.next().context("/dir needs a folder")?;
                options.dir = PathBuf::from(value);
            }
            "/payload" | "--payload" => {
                let value = args.next().context("/payload needs a folder")?;
                options.payload = PathBuf::from(value);
            }
            other if other.starts_with("/dir=") => {
                options.dir = PathBuf::from(&other[5..]);
            }
            other if other.starts_with("/payload=") => {
                options.payload = PathBuf::from(&other[9..]);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    if options.uninstall {
        return uninstall(&options);
    }
    install(&options)
}

/// Puts the program in place, and says what it did.
fn install(options: &Options) -> anyhow::Result<()> {
    let program = options.dir.join(EXE);
    if !options.payload.join(EXE).exists() {
        bail!(
            "the program to install is not beside the installer: {}",
            options.payload.display()
        );
    }
    std::fs::create_dir_all(&options.dir)
        .with_context(|| format!("making {}", options.dir.display()))?;

    // The files of the program, so the uninstaller knows what is ours.
    let mut installed: Vec<String> = Vec::new();
    copy_tree(&options.payload, &options.dir, &mut installed)?;
    let manifest = options.dir.join(".ely-installed");
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&installed)?,
    )
    .with_context(|| format!("writing {}", manifest.display()))?;

    if !options.no_associations {
        register(&program)?;
    }
    shortcuts(&options.dir, options.silent)?;
    uninstall_entry(&options.dir, &program)?;
    if !options.silent {
        println!("{APP} 已安装到 {}", options.dir.display());
    }
    Ok(())
}

/// Copies a folder, and remembers every file it wrote.
///
/// The names are relative to the folder the program goes into, so the
/// uninstaller finds them again where they are.
fn copy_tree(from: &Path, to: &Path, installed: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // The installer does not install itself.
        if name == "mpelyoffice-setup.exe" || name == "mpelyoffice-setup" {
            continue;
        }
        let source = entry.path();
        let target = to.join(name.as_ref());
        if source.is_dir() {
            std::fs::create_dir_all(&target)?;
            let mut deeper = Vec::new();
            copy_tree(&source, &target, &mut deeper)?;
            for path in deeper {
                installed.push(format!("{name}/{path}"));
            }
        } else {
            std::fs::copy(&source, &target)
                .with_context(|| format!("copying {}", source.display()))?;
            installed.push(name.to_string());
        }
    }
    Ok(())
}

/// Writes the file types, with the script the project keeps for them.
fn register(program: &Path) -> anyhow::Result<()> {
    let script = std::env::temp_dir().join("mpelyoffice-register.ps1");
    std::fs::write(&script, REGISTER)?;
    let output = Command::new("powershell")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg("-Exe")
        .arg(&program)
        .output()
        .context("running the file type script")?;
    let _ = std::fs::remove_file(&script);
    if !output.status.success() {
        bail!(
            "the file types were not written: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// The start menu and desktop entries.
fn shortcuts(dir: &Path, silent: bool) -> anyhow::Result<()> {
    let program = dir.join(EXE);
    let menu = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Microsoft/Windows/Start Menu/Programs")
        .join(APP);
    std::fs::create_dir_all(&menu)?;
    let script = format!(
        "$w = New-Object -ComObject WScript.Shell; \
         $s = $w.CreateShortcut('{link}'); $s.TargetPath = '{program}'; \
         $s.WorkingDirectory = '{dir}'; $s.Save(); \
         if ('{desktop}' -eq 'yes') {{ \
           $d = $w.CreateShortcut('{desktop_link}'); $d.TargetPath = '{program}'; \
           $d.WorkingDirectory = '{dir}'; $d.Save() }}",
        link = menu.join("MPelyoffice.lnk").display(),
        program = program.display(),
        dir = dir.display(),
        desktop = if silent { "no" } else { "yes" },
        desktop_link = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Desktop/MPelyoffice.lnk")
            .display(),
    );
    let path = std::env::temp_dir().join("mpelyoffice-shortcuts.ps1");
    std::fs::write(&path, script)?;
    let output = Command::new("powershell")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    if let Ok(output) = output {
        if !output.status.success() {
            eprintln!(
                "the shortcuts were not written: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(())
}

/// The entry the system's program list shows.
fn uninstall_entry(dir: &Path, program: &Path) -> anyhow::Result<()> {
    let key = format!("HKCU\\{UNINSTALL_KEY}");
    let output = Command::new("reg")
        .args(["add", &key, "/v", "DisplayName", "/t", "REG_SZ", "/d", APP, "/f"])
        .output()?;
    if !output.status.success() {
        bail!("the program list entry was not written");
    }
    for (name, value) in [
        ("DisplayVersion", env!("CARGO_PKG_VERSION").to_string()),
        ("Publisher", "MPelyoffice".to_string()),
        ("InstallLocation", dir.display().to_string()),
        (
            "UninstallString",
            format!("\"{}\" /uninstall", program.display()),
        ),
    ] {
        let output = Command::new("reg")
            .args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", &value, "/f"])
            .output()?;
        if !output.status.success() {
            bail!("the program list entry was not written");
        }
    }
    Ok(())
}

/// Takes the program away; the user's documents are not touched.
fn uninstall(options: &Options) -> anyhow::Result<()> {
    let dir = installed_dir().unwrap_or_else(|| options.dir.clone());
    let manifest = dir.join(".ely-installed");
    if manifest.exists() {
        let text = std::fs::read_to_string(&manifest)?;
        let files: Vec<String> = serde_json::from_str(&text)?;
        // The folders the files were in, deepest first, so they can go
        // once they are empty.
        let mut folders: Vec<PathBuf> = Vec::new();
        for file in files.iter() {
            let path = dir.join(file);
            if path.is_dir() {
                let _ = std::fs::remove_dir(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
            if let Some(parent) = path.parent() {
                // Every folder above the file, up to the program's own.
                let mut folder = Some(parent.to_path_buf());
                while let Some(current) = folder {
                    if current == dir {
                        break;
                    }
                    folders.push(current.clone());
                    folder = current.parent().map(|parent| parent.to_path_buf());
                }
            }
        }
        folders.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        folders.dedup();
        for folder in folders {
            // Only an empty folder goes; a folder of the user's own stays.
            let _ = std::fs::remove_dir(&folder);
        }
    }
    let _ = std::fs::remove_file(&manifest);
    // The file types, by the same script that wrote them.
    let script = std::env::temp_dir().join("mpelyoffice-unregister.ps1");
    std::fs::write(&script, REGISTER)?;
    let _ = Command::new("powershell")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg("-Exe")
        .arg(dir.join(EXE))
        .arg("-Uninstall")
        .output();
    let _ = std::fs::remove_file(&script);
    // The program list entry.
    let _ = Command::new("reg")
        .args(["delete", &format!("HKCU\\{UNINSTALL_KEY}"), "/f"])
        .output();
    // The shortcuts.
    let menu = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Microsoft/Windows/Start Menu/Programs")
        .join(APP);
    let _ = std::fs::remove_file(menu.join("MPelyoffice.lnk"));
    let _ = std::fs::remove_dir(&menu);
    let desktop = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Desktop/MPelyoffice.lnk");
    let _ = std::fs::remove_file(desktop);
    // The folder, when it is empty.
    let _ = std::fs::remove_dir(&dir);
    if !options.silent {
        println!("{APP} 已移除；用户的文档没有动。");
    }
    Ok(())
}
