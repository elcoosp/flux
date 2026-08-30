//! Pretty-printing for top-level [`crate::ast::Decl`] nodes.

use crate::ast::Annotation;
use crate::ast::ComponentDecl;
use crate::ast::ConstBinding;
use crate::ast::Decl;
use crate::ast::FnDecl;
use crate::ast::RecordDecl;
use crate::ast::TraitDecl;
use crate::ast::TypeDecl;
use crate::ast::UseDecl;
use crate::fmt::expr::write_block;
use crate::fmt::expr::write_expr;
use crate::fmt::expr::write_fn_name;
use crate::fmt::expr::write_indented_block;
use crate::fmt::indent_str;
use crate::fmt::ty::write_opt_generics;
use crate::fmt::ty::write_opt_param_list;
use crate::fmt::ty::write_type;

/// Appends `decl` at `indent` levels (top-level declarations use `indent = 0`).
pub(crate) fn write_decl(out: &mut String, decl: &Decl, indent: usize) {
    match decl {
        Decl::Use(use_decl) => write_use(out, use_decl),
        Decl::Component(component) => write_component(out, component, indent),
        Decl::Fn(fn_decl) => write_fn(out, fn_decl, indent),
        Decl::Type(type_decl) => write_type_decl(out, type_decl),
        Decl::Record(record) => write_record(out, record),
        Decl::Trait(trait_decl) => write_trait(out, trait_decl, indent),
        Decl::Capability(capability) => write_capability(out, capability, indent),
        Decl::Const(const_binding) => write_const(out, const_binding),
    }
}

/// Appends `@name(arg, arg)` annotations, each on its own line prefix.
fn write_annotations(out: &mut String, annotations: &[Annotation], indent: usize) {
    for annotation in annotations {
        out.push_str(&indent_str(indent));
        out.push('@');
        out.push_str(&annotation.name.name);
        if !annotation.args.is_empty() {
            out.push('(');
            for (index, arg) in annotation.args.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                match arg {
                    crate::ast::Arg::Positional(expr) => write_expr(out, expr, 0),
                    crate::ast::Arg::Named { name, value } => {
                        out.push_str(&name.name);
                        out.push_str(": ");
                        write_expr(out, value, 0);
                    }
                }
            }
            out.push(')');
        }
        out.push('\n');
    }
}

fn write_use(out: &mut String, use_decl: &UseDecl) {
    out.push_str("use ");
    for (index, segment) in use_decl.segments.iter().enumerate() {
        if index > 0 {
            out.push_str("::");
        }
        out.push_str(&segment.name);
    }
    if use_decl.glob {
        out.push_str("::*");
    }
}

fn write_component(out: &mut String, component: &ComponentDecl, indent: usize) {
    write_annotations(out, &component.annotations, indent);
    out.push_str(&indent_str(indent));
    out.push_str("compo ");
    out.push_str(&component.name.name);
    write_opt_generics(out, &component.generics);
    if !component.props.is_empty() {
        out.push('(');
        for (index, prop) in component.props.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&prop.name.name);
            out.push_str(": ");
            write_type(out, &prop.ty);
            if let Some(default) = &prop.default {
                out.push_str(" = ");
                write_expr(out, default, 0);
            }
        }
        out.push(')');
    }
    // The component body is an indented block (no braces) on the following lines.
    write_indented_block(out, &component.body, indent);
}

fn write_fn(out: &mut String, fn_decl: &FnDecl, indent: usize) {
    out.push_str(&indent_str(indent));
    out.push_str("fn ");
    write_fn_name(out, &fn_decl.name);
    write_opt_generics(out, &fn_decl.generics);
    write_opt_param_list(out, &fn_decl.params);
    if let Some(ret) = &fn_decl.ret {
        out.push_str(" -> ");
        write_type(out, ret);
    }
    // A `fn` body is a braced code block (`{ … }`), unlike a `compo` body.
    write_block(out, &fn_decl.body, indent, false);
}

fn write_type_decl(out: &mut String, type_decl: &TypeDecl) {
    out.push_str("type ");
    out.push_str(&type_decl.name.name);
    write_opt_generics(out, &type_decl.generics);
    out.push_str(" =\n");
    for (index, variant) in type_decl.variants.iter().enumerate() {
        if index == 0 {
            out.push_str("  | ");
        } else {
            out.push_str("\n  | ");
        }
        out.push_str(&variant.name.name);
        if !variant.fields.is_empty() {
            out.push('(');
            for (f_index, field) in variant.fields.iter().enumerate() {
                if f_index > 0 {
                    out.push_str(", ");
                }
                write_type(out, field);
            }
            out.push(')');
        }
    }
}

fn write_record(out: &mut String, record: &RecordDecl) {
    out.push_str("record ");
    out.push_str(&record.name.name);
    out.push_str(" {\n");
    for (index, field) in record.fields.iter().enumerate() {
        out.push_str("  ");
        out.push_str(&field.name.name);
        out.push_str(": ");
        write_type(out, &field.ty);
        if index + 1 < record.fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
}

fn write_trait(out: &mut String, trait_decl: &TraitDecl, indent: usize) {
    out.push_str(&indent_str(indent));
    out.push_str("trait ");
    out.push_str(&trait_decl.name.name);
    write_opt_generics(out, &trait_decl.generics);
    out.push_str(" {\n");
    let child = indent + 1;
    for method in &trait_decl.methods {
        out.push_str(&indent_str(child));
        out.push_str("fn ");
        write_fn_name(out, &method.name);
        write_opt_generics(out, &method.generics);
        write_opt_param_list(out, &method.params);
        if let Some(ret) = &method.ret {
            out.push_str(" -> ");
            write_type(out, ret);
        }
        out.push('\n');
    }
    out.push_str(&indent_str(indent));
    out.push('}');
}

fn write_capability(out: &mut String, capability: &crate::ast::CapabilityDecl, indent: usize) {
    out.push_str(&indent_str(indent));
    out.push_str("capability ");
    out.push_str(&capability.name.name);
    out.push_str(" {\n");
    let child = indent + 1;
    for method in &capability.methods {
        out.push_str(&indent_str(child));
        out.push_str("fn ");
        write_fn_name(out, &method.name);
        write_opt_generics(out, &method.generics);
        write_opt_param_list(out, &method.params);
        if let Some(ret) = &method.ret {
            out.push_str(" -> ");
            write_type(out, ret);
        }
        out.push('\n');
    }
    out.push_str(&indent_str(indent));
    out.push('}');
}

fn write_const(out: &mut String, const_binding: &ConstBinding) {
    for (index, segment) in const_binding.path.iter().enumerate() {
        if index > 0 {
            out.push('.');
        }
        out.push_str(&segment.name);
    }
    out.push_str(" = ");
    write_expr(out, &const_binding.value, 0);
}
