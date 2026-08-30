//! Pretty-printing for [`crate::ast::Type`] (type expressions and declarations).

use crate::ast::Type;
use crate::ast::TypeKindAst;

/// Appends `ty` to `out` as a type expression.
pub(crate) fn write_type(out: &mut String, ty: &Type) {
    match &ty.kind {
        TypeKindAst::Primitive(name) => out.push_str(name),
        TypeKindAst::Named { name, args } => {
            out.push_str(&name.name);
            if !args.is_empty() {
                out.push('[');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    write_type(out, arg);
                }
                out.push(']');
            }
        }
        TypeKindAst::Record(fields) => {
            out.push_str("{ ");
            for (index, (field, field_ty)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&field.name);
                out.push_str(": ");
                write_type(out, field_ty);
            }
            out.push_str(" }");
        }
        TypeKindAst::Fn { params, ret } => {
            out.push_str("Fn(");
            for (index, param) in params.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_type(out, param);
            }
            out.push_str(") -> ");
            write_type(out, ret);
        }
    }
}

/// Appends a parameter list `(a: Int, b: Int)`. The parentheses are always
/// emitted (a bare `fn name` with no `()` is not valid Flux surface; the parser
/// requires the `(`) so the round-trip is preserved.
pub(crate) fn write_opt_param_list(out: &mut String, params: &[crate::ast::Param]) {
    out.push('(');
    for (index, param) in params.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&param.name.name);
        if let Some(ty) = &param.ty {
            out.push_str(": ");
            write_type(out, ty);
        }
        if let Some(default) = &param.default {
            out.push_str(" = ");
            crate::fmt::expr::write_expr(out, default, 0);
        }
    }
    out.push(')');
}

/// Appends a generic parameter list `[T, U: Bound]`, eliding the brackets when empty.
pub(crate) fn write_opt_generics(out: &mut String, generics: &[crate::ast::TypeParam]) {
    if generics.is_empty() {
        return;
    }
    out.push('[');
    for (index, param) in generics.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&param.name.name);
        if let Some(bound) = &param.bound {
            out.push_str(": ");
            out.push_str(&bound.name);
        }
    }
    out.push(']');
}
