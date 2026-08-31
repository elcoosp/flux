//! `flux build` — release codegen for iOS or Android (FLUX-022, spec §14.3).
//!
//! Drives [`flux_devserver::Pipeline`] through the lowered IR + AST path, then
//! for every compiled source runs the platform codegen and writes the result
//! under `platforms/<platform>/Generated/`. The generated sources are written
//! FIRST (the spec's primary artifact); only then is the native toolchain
//! invoked as a release gate — a generated app that fails to compile makes
//! `flux build` exit non-zero with an actionable message (AGENTS.md §3.11).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};

use crate::Platform;

use crate::sources;

/// The resolved native-toolchain invocation for a platform: the argv to spawn
/// (`program` is `args[0]`) and the working directory the spawn runs in (so a
/// relative `-project`/`-workspace` or `gradlew` path resolves against the repo
/// root rather than the process CWD).
///
/// Injected so unit tests can drive `run` against a stubbed `xcodebuild` /
/// `gradlew` without shelling out to the real toolchain. When the resolution is
/// `None` the toolchain is considered absent and only the generated sources are
/// emitted (the CI-without-Xcode path).
pub(crate) type CommandResolver =
    dyn Fn(Platform, &Path) -> Option<(Vec<String>, std::path::PathBuf)>;

/// Runs release codegen for `platform` over the project rooted at `root`.
///
/// # Errors
///
/// Returns an error when no `.flux` sources are found, when the pipeline fails
/// to compile the project, when a generated source cannot be written, or when
/// the native toolchain is present and the generated app fails to compile.
pub(crate) fn run(platform: Platform, root: &Path) -> anyhow::Result<()> {
    run_with(platform, root, &resolve_toolchain)
}

/// Like [`run`] but injects a [`CommandResolver`] for the native-toolchain
/// spawn, so tests can stub `xcodebuild` / `gradlew` without touching `PATH`.
pub(crate) fn run_with(
    platform: Platform,
    root: &Path,
    resolver: &CommandResolver,
) -> anyhow::Result<()> {
    let sources_list = sources::gather(root)?;

    let mut pipeline = flux_devserver::Pipeline::new(root, false);
    for (path, source) in sources_list {
        pipeline.set_source(&path, source);
    }
    if let Err(diag) = pipeline.compile() {
        tracing::error!(message = %diag.message, "compile failed");
        bail!(
            "project at {} does not compile — {}",
            root.display(),
            diag.message
        );
    }

    let compiled = pipeline.compiled_sources();
    if compiled.is_empty() {
        bail!("project compiled but produced no sources to codegen");
    }

    // Generated sources are the spec's primary artifact — emit them BEFORE the
    // native gate so a triage engineer always has something to inspect.
    let out_dir = root
        .join("platforms")
        .join(platform.generated_dir_name())
        .join("Generated");
    // Clean the output dir so stale generated files from a previous run
    // (e.g. a second @main struct) don't collide with the new single-file
    // output and break the build.
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).context("cleaning stale generated sources dir")?;
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating generated sources dir {}", out_dir.display()))?;

    for (path, ir, ast) in &compiled {
        let component_code = match platform {
            Platform::Ios => flux_codegen_swift::codegen(ir, ast),
            Platform::Android => flux_codegen_kotlin::codegen(ir, ast),
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "generated".to_owned());
        let stem_owned = stem.clone();

        // Walk the bridge to find the root component's source name. The root
        // component is the last user-defined component in declaration order
        // (a Flux program's top-level component is declared last, after its
        // children). Sorting by source span gives deterministic declaration
        // order; HashMap iteration alone does not.
        let root_comp_name: String = {
            let bridge = flux_codegen_core::Bridge::build(ast);
            let mut comps: Vec<_> = bridge.components().collect();
            comps.sort_by_key(|(_, comp)| comp.span.start);
            comps
                .last()
                .map(|(_, comp)| comp.name.name.clone())
                .unwrap_or_else(|| stem_owned.clone())
        };

        // Emit a self-contained app file: prelude import + @main wrapper +
        // component, all in one file. For Swift, two files in the same module
        // cannot BOTH have top-level declarations — an import in one file makes
        // `@main` illegal in another. So we strip the prelude from the
        // component body and emit it ONCE at the top of the combined file.
        let app_code = match platform {
            Platform::Ios => {
                let prelude = "import SwiftUI\n";
                let body = if component_code.starts_with(prelude) {
                    component_code[prelude.len()..].trim_start().to_owned()
                } else {
                    component_code.clone()
                };
                format!(
                    "{prelude}\n@main\nstruct {root_comp_name}App: SwiftUI.App {{\n    var body: some Scene {{\n        WindowGroup {{\n            {root_comp_name}()\n        }}\n    }}\n}}\n\n{body}"
                )
            }
            Platform::Android => {
                // Strip the package decl + imports from the component body
                // (the entry point owns them) so a single-file app compiles.
                let body = component_code
                    .lines()
                    .skip_while(|l| l.starts_with("package ") || l.starts_with("import ") || l.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "// Generated entry point\npackage dev.flux.app\n\nimport android.os.Bundle\nimport androidx.activity.ComponentActivity\nimport androidx.activity.compose.setContent\nimport androidx.compose.material3.MaterialTheme\n\nclass MainActivity : ComponentActivity() {{\n    override fun onCreate(savedInstanceState: Bundle?) {{\n        super.onCreate(savedInstanceState)\n        setContent {{\n            MaterialTheme {{\n                {root_comp_name}()\n            }}\n        }}\n    }}\n}}\n\n{body}"
                )
            }
        };
        let file_name = match platform {
            Platform::Ios => format!("{root_comp_name}App.{}", platform.source_extension()),
            Platform::Android => "MainActivity.kt".to_owned(),
        };
        let out = out_dir.join(&file_name);
        std::fs::write(&out, app_code)
            .with_context(|| format!("writing generated source {}", out.display()))?;
        tracing::info!(source = %path.display(), target = %out.display(), "codegen");
    }

    println!(
        "generated {} source(s) under {}",
        compiled.len(),
        out_dir.display()
    );

    invoke_native_toolchain(platform, root, resolver)?;
    Ok(())
}

/// Invokes the native toolchain when present and gates on its result.
///
/// The native build is the release gate: a present toolchain that exits
/// non-zero means the generated app does not compile, so `flux build` must
/// fail (production-readiness contract). When the toolchain is absent we keep
/// the previous emit-only behavior and log which verification was SKIPPED, so
/// CI without Xcode/Gradle still produces sources instead of hard-failing.
fn invoke_native_toolchain(
    platform: Platform,
    root: &Path,
    resolver: &CommandResolver,
) -> anyhow::Result<()> {
    let Some((args, cwd)) = resolver(platform, root) else {
        // Toolchain absent → emit-only, but record the skipped verification and
        // hand back the exact manual command so CI/local devs can finish the
        // gate themselves (AGENTS.md §0.4). This is the manual-command fallback.
        match platform {
            Platform::Ios => {
                let project = root.join("runtimes").join("ios").join("FluxApp.xcodeproj");
                println!(
                    "warning: xcodebuild not found — emitted generated sources only.\n  \
                     to finish the iOS release gate manually:\n    xcodebuild \
                     -project {} -scheme FluxApp -configuration Release \
                     -destination 'generic/platform=iOS' build",
                    project.display()
                );
            }
            Platform::Android => {
                let gradlew = root.join("gradlew");
                println!(
                    "warning: gradle not found — emitted generated sources only.\n  \
                     to finish the Android release gate manually:\n    {} \
                     :runtimes:android:app:assembleDebug",
                    gradlew.display()
                );
            }
        }
        return Ok(());
    };

    let Some((program, cmd_args)) = args.split_first() else {
        bail!(
            "internal error: resolved toolchain command for {platform:?} was empty",
            platform = platform
        );
    };
    tracing::info!(program = %program, cwd = %cwd.display(), "invoking native toolchain");

    let output = Command::new(program)
        .args(cmd_args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("spawning native toolchain `{program}`"))?;

    if output.status.success() {
        return Ok(());
    }

    // Surface a bounded, actionable tail of the build log (what/where/how).
    let log = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = log.lines().rev().take(20).collect();
    let tail = tail.iter().rev().copied().collect::<Vec<_>>().join("\n");
    let code = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_owned());
    let platform_name = match platform {
        Platform::Ios => "iOS",
        Platform::Android => "Android",
    };
    bail!(
        "native build failed (exit {code}) for {platform_name} — generated sources are at {}\n\
         build log (tail):\n{tail}",
        root.join("platforms")
            .join(platform.generated_dir_name())
            .join("Generated")
            .display()
    );
}

/// The on-disk, repo-root-relative path of the real consumer app that wraps the
/// generated sources for `platform`. `flux build` only invokes the native
/// toolchain when this project actually exists — the generated-sources dir
/// (`platforms/<platform>/Generated`, which `run` itself creates) is NOT a
/// buildable app, so probing it would make the release gate fire spuriously.
fn app_project_dir(platform: Platform, root: &Path) -> Option<std::path::PathBuf> {
    let dir = match platform {
        // The iOS consumer app is a standalone XcodeGen project at `runtimes/ios`
        // (project.yml + FluxApp.xcodeproj); it imports the generated Swift via
        // its own Sources glob. The `platforms/ios` dir is only generated output.
        Platform::Ios => root.join("runtimes").join("ios"),
        // Android shares the repo-root Gradle build; the app module lives under
        // `runtimes/android/app` and is addressed by the `:runtimes:android:app`
        // Gradle path. `platforms/android` is only generated output.
        Platform::Android => root.join("runtimes").join("android").join("app"),
    };
    dir.exists().then_some(dir)
}

/// The default resolver: finds `xcodebuild` / `./gradlew` on `PATH` or the repo
/// root and returns the concrete command to spawn.
///
/// The native build is only attempted when a real consumer-app project exists
/// for the platform (see [`app_project_dir`]); otherwise the resolver returns
/// `None` and `flux build` stays in emit-only mode (the CI-without-Xcode path).
fn resolve_toolchain(platform: Platform, root: &Path) -> Option<(Vec<String>, std::path::PathBuf)> {
    match platform {
        Platform::Ios => {
            // Only fire when both the Xcode app project and `xcodebuild` exist.
            // The app project is `runtimes/ios` (project.yml + FluxApp.xcodeproj).
            let app_dir = app_project_dir(platform, root);
            let project_arg = app_dir.as_ref().map(|dir| {
                if dir.join("FluxApp.xcworkspace").exists() {
                    format!("-workspace={}", dir.join("FluxApp.xcworkspace").display())
                } else if dir.join("FluxApp.xcodeproj").exists() {
                    format!("-project={}", dir.join("FluxApp.xcodeproj").display())
                } else {
                    // XcodeGen manifest present — point xcodebuild at the generated
                    // project dir so it discovers FluxApp.xcodeproj after `xcodegen`.
                    format!("-project={}", dir.join("FluxApp.xcodeproj").display())
                }
            });
            let has_app = app_dir
                .as_ref()
                .map(|dir| {
                    dir.join("project.yml").exists()
                        || dir.join("FluxApp.xcodeproj").exists()
                        || dir.join("FluxApp.xcworkspace").exists()
                })
                .unwrap_or(false);
            if let (Some(project_arg), Some(cwd)) = (project_arg, app_dir) {
                if has_app && which("xcodebuild") {
                    Some((
                        vec![
                            "xcodebuild".to_owned(),
                            project_arg,
                            "-scheme".to_owned(),
                            "FluxApp".to_owned(),
                            "-configuration".to_owned(),
                            "Release".to_owned(),
                            "-destination".to_owned(),
                            "generic/platform=iOS".to_owned(),
                            "build".to_owned(),
                        ],
                        cwd,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        }
        Platform::Android => app_project_dir(platform, root)
            .and_then(|_| find_gradlew(root))
            .map(|gradlew| {
                (
                    vec![gradlew, ":runtimes:android:app:assembleDebug".to_owned()],
                    root.to_path_buf(),
                )
            }),
    }
}

/// Finds `./gradlew` at the repo root (preferred) or `gradle` on `PATH`.
fn find_gradlew(root: &Path) -> Option<String> {
    let local = root.join("gradlew");
    if local.exists() {
        return Some(local.to_string_lossy().into_owned());
    }
    if which("gradle") {
        return Some("gradle".to_owned());
    }
    None
}

/// Returns `true` when `name` resolves on `PATH`.
fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).exists()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_fixture(dir: &Path, body: &str) {
        std::fs::write(dir.join("main.flux"), body).unwrap();
    }

    /// Writes an executable shell stub and returns its path. The file handle is
    /// dropped before returning so the stub is not left open for writing —
    /// spawning it again must not hit `ETXTBSY` ("Text file busy") on Linux,
    /// which refuses to `execve` a file that is still open for write. macOS
    /// does not enforce this, so the bug only surfaces on the `ubuntu-latest`
    /// CI runner, not locally.
    fn create_stub(dir: &Path, name: &str, body: &str) -> String {
        let stub = dir.join(name);
        {
            let mut f = std::fs::File::create(&stub).expect("create stub script");
            writeln!(f, "{body}").expect("write stub script");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&stub)
                .expect("stat stub script")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub, perms).expect("chmod stub script");
        }
        stub.to_string_lossy().into_owned()
    }

    #[test]
    fn app_project_dir_uses_real_ios_app_not_generated_dir() {
        // The consumer app is at `runtimes/ios`, NOT `platforms/ios` (which is
        // only generated output that `run` itself creates). The resolver must
        // probe the real app so the release gate fires on a buildable project.
        let tmp = std::env::temp_dir().join(format!(
            "flux-build-appdir-{}-{}",
            std::process::id(),
            "ios"
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Only the generated dir exists — must NOT be treated as an app.
        std::fs::create_dir_all(tmp.join("platforms").join("ios")).unwrap();
        assert!(
            app_project_dir(Platform::Ios, &tmp).is_none(),
            "platforms/ios is generated output, not a buildable app"
        );
        // The real app project exists → resolves.
        std::fs::create_dir_all(tmp.join("runtimes").join("ios")).unwrap();
        assert!(
            app_project_dir(Platform::Ios, &tmp).is_some(),
            "runtimes/ios is the real consumer app"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolver_fires_only_with_real_ios_app() {
        // Regression: the old resolver probed `platforms/ios` (only generated
        // output that `run` itself creates) and would fire merely because sources
        // had been emitted — before any real app existed — making the release
        // gate fire spuriously. The resolver must return None when only the
        // generated dir is present, and only resolve to a `xcodebuild` command
        // once a real `runtimes/ios` app project exists AND `xcodebuild` is on
        // PATH (neither env mutation nor a real Xcode install is assumed here).
        let tmp = std::env::temp_dir().join(format!(
            "flux-build-resgen-{}-{}",
            std::process::id(),
            "ios"
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Only the generated dir exists → never fire.
        std::fs::create_dir_all(tmp.join("platforms").join("ios")).unwrap();
        assert_eq!(
            resolve_toolchain(Platform::Ios, &tmp),
            None,
            "resolver must not fire on generated-only dir"
        );
        // The real app project exists → resolve iff xcodebuild is available.
        std::fs::create_dir_all(tmp.join("runtimes").join("ios")).unwrap();
        std::fs::write(
            tmp.join("runtimes").join("ios").join("project.yml"),
            "name: FluxApp\n",
        )
        .unwrap();
        let expected = if which("xcodebuild") {
            Some((
                vec![
                    "xcodebuild".to_owned(),
                    format!(
                        "-project={}",
                        tmp.join("runtimes")
                            .join("ios")
                            .join("FluxApp.xcodeproj")
                            .display()
                    ),
                    "-scheme".to_owned(),
                    "FluxApp".to_owned(),
                    "-configuration".to_owned(),
                    "Release".to_owned(),
                    "-destination".to_owned(),
                    "generic/platform=iOS".to_owned(),
                    "build".to_owned(),
                ],
                tmp.join("runtimes").join("ios"),
            ))
        } else {
            None
        };
        assert_eq!(resolve_toolchain(Platform::Ios, &tmp), expected);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn android_resolver_uses_local_gradlew() {
        // With a local `./gradlew` and the `runtimes/android/app` module, the
        // resolver must return the assembleDebug command targeting that module,
        // run from the repo root.
        let tmp = std::env::temp_dir().join(format!(
            "flux-build-android-{}-{}",
            std::process::id(),
            "res"
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        create_stub(&tmp, "gradlew", "#!/bin/sh\nexit 0\n");
        std::fs::create_dir_all(tmp.join("runtimes").join("android").join("app")).unwrap();
        let resolved = resolve_toolchain(Platform::Android, &tmp);
        assert_eq!(
            resolved,
            Some((
                vec![
                    tmp.join("gradlew").to_string_lossy().into_owned(),
                    ":runtimes:android:app:assembleDebug".to_owned()
                ],
                tmp.clone()
            ))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn absent_toolchain_emits_and_returns_ok() {
        // No xcodebuild/gradle on PATH and no platforms/ios scheme dir → the
        // resolver returns None → run() must still succeed after emitting.
        let tmp = std::env::temp_dir().join(format!("flux-build-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_fixture(
            &tmp,
            "compo Main\n  state count: Int = 0\n\n  Text(text: \"hi\")\n",
        );

        let resolver: Box<CommandResolver> = Box::new(|_, _| None);
        let result = run_with(Platform::Ios, &tmp, &*resolver);
        assert!(result.is_ok(), "absent toolchain must not fail: {result:?}");
        let generated = tmp
            .join("platforms")
            .join("ios")
            .join("Generated")
            .join("MainApp.swift");
        assert!(
            generated.exists(),
            "generated sources must be emitted first"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn failing_toolchain_makes_build_fail() {
        // A stub that exits 1 (simulating a broken generated app) must make
        // run() return Err, not Ok.
        let tmp = std::env::temp_dir().join(format!("flux-build-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // platforms/ios must exist so the iOS resolver proceeds to spawn.
        std::fs::create_dir_all(tmp.join("runtimes").join("ios")).unwrap();
        write_fixture(
            &tmp,
            "compo Main\n  state count: Int = 0\n\n  Text(text: \"hi\")\n",
        );
        let stub = create_stub(
            &tmp.join("runtimes").join("ios"),
            "xcodebuild",
            "#!/bin/sh\nexit 1\n",
        );
        let cwd = tmp.clone();
        let resolver: Box<CommandResolver> = Box::new(move |_, _| {
            Some((
                vec![
                    stub.clone(),
                    "-project".to_owned(),
                    "FluxApp.xcodeproj".to_owned(),
                    "-scheme".to_owned(),
                    "FluxApp".to_owned(),
                    "-configuration".to_owned(),
                    "Release".to_owned(),
                    "-destination".to_owned(),
                    "generic/platform=iOS".to_owned(),
                    "build".to_owned(),
                ],
                cwd.join("runtimes").join("ios"),
            ))
        });
        let result = run_with(Platform::Ios, &tmp, &*resolver);
        assert!(result.is_err(), "failing toolchain must fail: {result:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn passing_toolchain_returns_ok() {
        // A stub that exits 0 (simulating a healthy generated app) must make
        // run() return Ok.
        let tmp = std::env::temp_dir().join(format!("flux-build-pass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::create_dir_all(tmp.join("platforms").join("ios")).unwrap();
        std::fs::create_dir_all(tmp.join("runtimes").join("ios")).unwrap();
        write_fixture(
            &tmp,
            "compo Main\n  state count: Int = 0\n\n  Text(text: \"hi\")\n",
        );
        let stub = create_stub(
            &tmp.join("runtimes").join("ios"),
            "xcodebuild",
            "#!/bin/sh\nexit 0\n",
        );
        let cwd = tmp.clone();
        let resolver: Box<CommandResolver> = Box::new(move |_, _| {
            Some((
                vec![
                    stub.clone(),
                    "-project".to_owned(),
                    "FluxApp.xcodeproj".to_owned(),
                    "-scheme".to_owned(),
                    "FluxApp".to_owned(),
                    "-configuration".to_owned(),
                    "Release".to_owned(),
                    "-destination".to_owned(),
                    "generic/platform=iOS".to_owned(),
                    "build".to_owned(),
                ],
                cwd.join("runtimes").join("ios"),
            ))
        });
        let result = run_with(Platform::Ios, &tmp, &*resolver);
        assert!(result.is_ok(), "passing toolchain must succeed: {result:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
