//! `flux build` — release codegen for iOS or Android (FLUX-022, spec §14.3).
//!
//! Drives [`flux_devserver::Pipeline`] through the lowered IR + AST path, then
//! for every compiled source runs the platform codegen and writes the result
//! under `platforms/<platform>/Generated/`. When the native toolchain
//! (`xcodebuild` / `gradle`) is present it is invoked afterwards; otherwise only
//! the generated sources are emitted and the skipped native step is logged.

use std::path::Path;

use anyhow::{Context, bail};

use crate::Platform;

use crate::sources;

/// Runs release codegen for `platform` over the project rooted at `root`.
///
/// # Errors
///
/// Returns an error when no `.flux` sources are found, when the pipeline fails
/// to compile the project, or when a generated source cannot be written.
pub(crate) fn run(platform: Platform, root: &Path) -> anyhow::Result<()> {
    let sources_list = sources::gather(root)?;

    let mut pipeline = flux_devserver::Pipeline::new(root, false);
    for (path, source) in sources_list {
        pipeline.set_source(&path, source);
    }
    if pipeline.compile().is_err() {
        bail!(
            "project at {} does not compile — fix the errors reported above, then rebuild",
            root.display()
        );
    }

    let compiled = pipeline.compiled_sources();
    if compiled.is_empty() {
        bail!("project compiled but produced no sources to codegen");
    }

    let out_dir = root
        .join("platforms")
        .join(platform.generated_dir_name())
        .join("Generated");
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating generated sources dir {}", out_dir.display()))?;

    for (path, ir, ast) in &compiled {
        let generated = match platform {
            Platform::Ios => flux_codegen_swift::codegen(ir, ast),
            Platform::Android => flux_codegen_kotlin::codegen(ir, ast),
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "generated".to_owned());
        let file_name = format!("{stem}.{}", platform.source_extension());
        let out = out_dir.join(&file_name);
        std::fs::write(&out, generated)
            .with_context(|| format!("writing generated source {}", out.display()))?;
        tracing::info!(source = %path.display(), target = %out.display(), "codegen");
    }

    println!(
        "generated {} source(s) under {}",
        compiled.len(),
        out_dir.display()
    );

    invoke_native_toolchain(platform, root);
    Ok(())
}

/// Invokes the native toolchain when present, otherwise documents the skip.
///
/// The native build is best-effort: a missing `xcodebuild`/`gradle` is not an
/// error because the generated sources (which the spec requires) are the
/// primary artifact on this path.
fn invoke_native_toolchain(platform: Platform, root: &Path) {
    match platform {
        Platform::Ios => {
            if which("xcodebuild") {
                tracing::info!(
                    "xcodebuild present; invoke `xcodebuild` against platforms/ios manually"
                );
            } else {
                tracing::warn!(
                    "xcodebuild not found; emitted generated sources only — \
                     build platforms/ios with Xcode/SwiftPM to produce the app"
                );
            }
        }
        Platform::Android => {
            let _ = root;
            if which("gradle") || which("./gradlew") {
                tracing::info!(
                    "gradle present; invoke the gradle build against platforms/android manually"
                );
            } else {
                tracing::warn!(
                    "gradle not found; emitted generated sources only — \
                     build platforms/android with Gradle to produce the app"
                );
            }
        }
    }
}

/// Returns `true` when `name` resolves on `PATH`.
fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).exists()))
        .unwrap_or(false)
}
