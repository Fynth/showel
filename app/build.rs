use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let workspace_root = manifest_dir
        .parent()
        .ok_or("app crate is expected to live inside the workspace root")?
        .to_path_buf();
    let styles_root = workspace_root.join("styles");
    let scss_entry = styles_root.join("app.scss");
    let output_css = manifest_dir.join("assets").join("app.css");

    emit_rerun_if_changed(&styles_root)?;
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("build.rs").display()
    );

    let css = grass::from_path(
        &scss_entry,
        &grass::Options::default().style(grass::OutputStyle::Expanded),
    )?;

    write_generated_css(&output_css, &css)?;
    configure_windows_resources(&workspace_root)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn configure_windows_resources(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    let icon_path = workspace_root.join("icons").join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let icon_path = icon_path.to_string_lossy().into_owned();
    let mut resources = winres::WindowsResource::new();
    resources.set_icon(&icon_path);
    resources.compile()?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn configure_windows_resources(_workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn emit_rerun_if_changed(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.is_file() {
        println!("cargo:rerun-if-changed={}", path.display());
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_if_changed(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    Ok(())
}

fn write_generated_css(path: &Path, css: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let needs_write = match fs::read_to_string(path) {
        Ok(existing) => existing != css,
        Err(_) => true,
    };

    if needs_write {
        fs::write(path, css)?;
    }

    Ok(())
}
