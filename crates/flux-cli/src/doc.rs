//! `flux doc` — emit a JSON schema of the stdlib API (FLUX-022, spec §14.3).
//!
//! Parses every `.flux` file in the repository `stdlib/` directory and reflects
//! each top-level declaration into a stable, machine-readable JSON document
//! describing the public stdlib API (components, functions, types, traits,
//! capabilities and constants). The emitted JSON is validated by the test
//! suite with `serde_json::from_str`.

use std::path::Path;

use anyhow::{Context, bail};

use flux_parser::{Decl, parse};

/// The default repository-relative path to the stdlib sources.
const STDLIB_DIR: &str = "stdlib";

/// Emits the stdlib API JSON schema to stdout.
///
/// # Errors
///
/// Returns an error when the `stdlib/` directory cannot be read, contains no
/// `.flux` files, or when any file fails to parse.
pub(crate) fn run() -> anyhow::Result<()> {
    let schema = build_schema(Path::new(STDLIB_DIR))?;
    let json =
        serde_json::to_string_pretty(&schema).context("serializing the stdlib schema to JSON")?;
    println!("{json}");
    Ok(())
}

/// Builds the stdlib API schema from the `.flux` files under `stdlib_dir`.
///
/// # Errors
///
/// Returns an error when the directory cannot be read, is empty of `.flux`
/// sources, or when any source does not parse.
pub fn build_schema(stdlib_dir: &Path) -> anyhow::Result<StdlibSchema> {
    let mut files: Vec<_> = std::fs::read_dir(stdlib_dir)
        .with_context(|| format!("reading stdlib dir {}", stdlib_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("flux"))
        .collect();
    files.sort();
    if files.is_empty() {
        bail!("no .flux files found in {}", stdlib_dir.display());
    }

    let mut modules = Vec::with_capacity(files.len());
    for (index, path) in files.iter().enumerate() {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("reading stdlib file {}", path.display()))?;
        let display = path.display().to_string();
        let ast = parse(&source, index as u32, &display)
            .with_context(|| format!("parsing stdlib file {}", path.display()))?;

        let mut items = Vec::with_capacity(ast.decls.len());
        for decl in &ast.decls {
            items.push(describe_decl(decl));
        }
        modules.push(Module {
            file: display,
            items,
        });
    }

    Ok(StdlibSchema {
        schema_version: 1,
        modules,
    })
}

/// Reflects one top-level declaration into its JSON shape.
fn describe_decl(decl: &Decl) -> DeclSchema {
    match decl {
        Decl::Component(c) => DeclSchema {
            kind: "component".to_owned(),
            name: c.name.name.clone(),
            props: c.props.iter().map(|p| p.name.name.clone()).collect(),
        },
        Decl::Fn(f) => DeclSchema {
            kind: "function".to_owned(),
            name: f.name.text.clone(),
            props: Vec::new(),
        },
        Decl::Type(t) => DeclSchema {
            kind: "type".to_owned(),
            name: t.name.name.clone(),
            props: t.variants.iter().map(|v| v.name.name.clone()).collect(),
        },
        Decl::Trait(t) => DeclSchema {
            kind: "trait".to_owned(),
            name: t.name.name.clone(),
            props: t.methods.iter().map(|m| m.name.text.clone()).collect(),
        },
        Decl::Capability(c) => DeclSchema {
            kind: "capability".to_owned(),
            name: c.name.name.clone(),
            props: c.methods.iter().map(|m| m.name.text.clone()).collect(),
        },
        Decl::Const(c) => DeclSchema {
            kind: "const".to_owned(),
            name: c.path.last().map(|i| i.name.clone()).unwrap_or_default(),
            props: Vec::new(),
        },
        Decl::Import(i) => DeclSchema {
            kind: "import".to_owned(),
            name: i.name.name.clone(),
            props: Vec::new(),
        },
        Decl::Use(u) => DeclSchema {
            kind: "use".to_owned(),
            name: u
                .segments
                .last()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            props: Vec::new(),
        },
        // `Decl` is `#[non_exhaustive]`; future variants reflect as `unknown`.
        _ => DeclSchema {
            kind: "unknown".to_owned(),
            name: String::new(),
            props: Vec::new(),
        },
    }
}

/// A machine-readable schema of the Flux stdlib API.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StdlibSchema {
    /// Schema document format version.
    pub schema_version: u32,
    /// One entry per parsed stdlib module.
    pub modules: Vec<Module>,
}

/// The reflected declarations of one stdlib module.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Module {
    /// The module's file path (relative to the repository root).
    pub file: String,
    /// The declarations the module contributes to the public API.
    pub items: Vec<DeclSchema>,
}

/// One top-level declaration reflected into JSON.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DeclSchema {
    /// The declaration kind: `component`, `function`, `type`, `trait`, ….
    pub kind: String,
    /// The declaration's primary name.
    pub name: String,
    /// Kind-specific member names (props, variants, or methods).
    pub props: Vec<String>,
}
