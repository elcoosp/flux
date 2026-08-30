//! `base?.field` (null-safe) access inference, extracted from `Checker::infer_inner`.
//!
//! Kept in its own module so `infer_inner` stays under the file-length gate.
//! The base type must be `Option[T]`; the result widens the field's type to
//! `Option[...]` because the chain short-circuits to `Null` when the base is
//! `Null`.
use super::*;

impl Checker {
    /// Resolves `base?.field` (FLUX null-safe field access).
    pub(crate) fn infer_opt_field_access(
        &mut self,
        base: &Expr,
        field: &Ident,
    ) -> Result<TcType, TypeError> {
        // Null-safe access (FLUX-053 / ADR-0051). The base type must be
        // `Option[T]`; the result widens the field's type to
        // `Option[...]` because the chain short-circuits to `Null` when
        // the base is `Null`.
        let base_ty = self.infer(base)?;
        let base_ty = self.resolve(&base_ty);
        match &base_ty {
            TcType::Option(inner) => {
                // Access the field on the unwrapped inner type, then
                // wrap the result back into `Option`.
                let inner_ty = match &**inner {
                    TcType::Record(fields) => {
                        if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field.name) {
                            (**ty).clone()
                        } else {
                            return Err(TypeError::new(
                                format!("no field `{}` on record", field.name),
                                field.span,
                            )
                            .with_hint(format!(
                                "record has fields: {}",
                                fields
                                    .iter()
                                    .map(|(n, _)| n.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )));
                        }
                    }
                    // Opaque inner types expose an open surface, like
                    // `Field` above: the accessed type is a fresh var,
                    // widened back into `Option`.
                    TcType::Var(_)
                    | TcType::Constrained(_, _)
                    | TcType::Named(_, _)
                    | TcType::Variant(_, _)
                    | TcType::Fn(_, _)
                    | TcType::List(_)
                    | TcType::Map(_, _) => self.fresh_ty(),
                    other => {
                        return Err(TypeError::new(
                            format!("cannot access field `{}` on `{}`", field.name, other),
                            field.span,
                        )
                        .with_hint("optional field access requires a record type".to_owned()));
                    }
                };
                Ok(TcType::Option(Box::new(inner_ty)))
            }
            // A base already known to be non-nullable (concrete record
            // or scalar) cannot be null-safe-chained: `?.` is only
            // meaningful over `Option`.
            TcType::Record(_) | TcType::Int | TcType::Bool | TcType::Float | TcType::String => Err(
                TypeError::new("`?.` requires an Option base".to_owned(), base.span).with_hint(
                    "optional chaining can only be applied to a nullable (Option) value".to_owned(),
                ),
            ),
            // Unresolved / opaque non-Option bases: be permissive and
            // return `Option[fresh]` so adapter-type chains don't reject
            // otherwise well-formed programs (mirrors `Field`).
            TcType::Var(_)
            | TcType::Constrained(_, _)
            | TcType::Named(_, _)
            | TcType::Variant(_, _)
            | TcType::Fn(_, _)
            | TcType::List(_)
            | TcType::Map(_, _) => Ok(TcType::Option(Box::new(self.fresh_ty()))),
            other => Err(TypeError::new(
                format!("`?.` requires an Option base, found `{}`", other),
                base.span,
            )
            .with_hint(
                "optional chaining can only be applied to a nullable (Option) value".to_owned(),
            )),
        }
    }
}
