//! `base.field` access inference, extracted from `Checker::infer_inner`.
//!
//! Kept in its own module so `infer_inner` stays under the file-length gate.
//! Records the resolved positional index into `field_indices` (keyed by the
//! expression's `NodeId`) so the bytecode emitter can emit `GET_FIELD` with the
//! slot the VM expects (records are positional).
use super::*;

impl Checker {
    /// Resolves `base.field` (FLUX record field access).
    pub(crate) fn infer_field_access(
        &mut self,
        base: &Expr,
        field: &Ident,
        expr: &Expr,
    ) -> Result<TcType, TypeError> {
        // Module-level associated constant access, e.g. `Color.red`:
        // the base is an identifier and the dot-path names a constant
        // registered under `"Color.red"`.
        if let ExprKind::Ident(base_ident) = &base.kind {
            let const_name = format!("{}.{}", base_ident.name, field.name);
            if let Some(Binding::Mono(ty)) = self.env.lookup(&const_name) {
                return Ok(ty.clone());
            }
        }
        let base_ty = self.infer(base)?;
        let base_ty = self.resolve(&base_ty);
        match &base_ty {
            TcType::Record(fields) => {
                if let Some((pos, (_, ty))) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (n, _))| n == &field.name)
                {
                    // Record the resolved positional index so the
                    // bytecode emitter can emit GET_FIELD with the slot
                    // the VM expects (records are positional).
                    let fid = compute_node_id(0, ExprTag(10), expr.span, None);
                    self.field_indices.insert(fid, pos as u16);
                    Ok((**ty).clone())
                } else {
                    Err(
                        TypeError::new(format!("no field `{}` on record", field.name), field.span)
                            .with_hint(format!(
                                "record has fields: {}",
                                fields
                                    .iter()
                                    .map(|(n, _)| n.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )),
                    )
                }
            }
            TcType::Named(name, _) => {
                // A named record type: resolve its fields from the
                // registered record constructor so field access (and
                // the bytecode field index) works through the nominal
                // type, not just the structural `Record` form.
                if let Some(Binding::Ctor(CtorKind::Record { fields })) = self.env.lookup(name) {
                    if let Some((pos, (_, ty))) = fields
                        .iter()
                        .enumerate()
                        .find(|(_, (n, _))| n == &field.name)
                    {
                        let fid = compute_node_id(0, ExprTag(10), expr.span, None);
                        self.field_indices.insert(fid, pos as u16);
                        Ok(ty.clone())
                    } else {
                        Err(TypeError::new(
                            format!("no field `{}` on record `{name}`", field.name),
                            field.span,
                        )
                        .with_hint(format!(
                            "record has fields: {}",
                            fields
                                .iter()
                                .map(|(n, _)| n.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )))
                    }
                } else {
                    Ok(self.fresh_ty())
                }
            }
            TcType::Var(_)
            | TcType::Constrained(_, _)
            | TcType::Variant(_, _)
            | TcType::Fn(_, _)
            | TcType::List(_)
            | TcType::Option(_)
            | TcType::Map(_, _) => Ok(self.fresh_ty()),
            other => Err(TypeError::new(
                format!("cannot access field `{}` on `{other}`", field.name),
                field.span,
            )
            .with_hint("field access requires a record type".to_owned())),
        }
    }
}
