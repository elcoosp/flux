//! Unification for the internal [`TcType`], with occurs-check.

use crate::kind::TcType;
use std::collections::HashMap;

/// Why unification failed.
#[derive(Clone, Debug, PartialEq)]
pub enum UnifyError {
    /// The two types could not be made equal.
    Mismatch(TcType, TcType),
    /// Unifying a variable with a type that contains it (infinite type).
    Recursive(TcType),
}

/// Fully applies the current substitution to `ty`.
fn resolve(ty: &TcType, subst: &HashMap<u32, TcType>) -> TcType {
    match ty {
        TcType::Var(id) => match subst.get(id) {
            Some(inner) => resolve(inner, subst),
            None => TcType::Var(*id),
        },
        TcType::Constrained(id, traits) => match subst.get(id) {
            Some(inner) => resolve(inner, subst),
            None => TcType::Constrained(*id, traits.clone()),
        },
        TcType::List(inner) => TcType::List(Box::new(resolve(inner, subst))),
        TcType::Option(inner) => TcType::Option(Box::new(resolve(inner, subst))),
        TcType::Map(k, v) => TcType::Map(Box::new(resolve(k, subst)), Box::new(resolve(v, subst))),
        TcType::Fn(params, ret) => TcType::Fn(
            params.iter().map(|p| resolve(p, subst)).collect(),
            Box::new(resolve(ret, subst)),
        ),
        TcType::Record(fields) => TcType::Record(
            fields
                .iter()
                .map(|(n, ty)| (n.clone(), Box::new(resolve(ty, subst))))
                .collect(),
        ),
        TcType::Variant(name, payload) => TcType::Variant(
            name.clone(),
            payload.iter().map(|t| resolve(t, subst)).collect(),
        ),
        TcType::Named(name, args) => TcType::Named(
            name.clone(),
            args.iter().map(|t| resolve(t, subst)).collect(),
        ),
        other => other.clone(),
    }
}

/// Unifies `a` and `b`, extending `subst` in place.
///
/// # Errors
///
/// Returns [`UnifyError::Mismatch`] when the types are incompatible, or
/// [`UnifyError::Recursive`] on an occurs-check failure.
pub(crate) fn unify_into(
    a: &TcType,
    b: &TcType,
    subst: &mut HashMap<u32, TcType>,
) -> Result<(), UnifyError> {
    let a = resolve(a, subst);
    let b = resolve(b, subst);
    match (&a, &b) {
        (TcType::Var(i), _) => bind(*i, &b, subst),
        (_, TcType::Var(i)) => bind(*i, &a, subst),
        // Two constrained variables with the same id are trivially equal.
        (TcType::Constrained(i, _), TcType::Constrained(j, _)) if i == j => Ok(()),
        // A constrained variable unified with a concrete type (or a different
        // constrained variable, or a plain variable): bind the constrained id
        // to the other type. The `i == j` self case is handled above.
        (TcType::Constrained(i, _), other) => bind(*i, other, subst),
        (other, TcType::Constrained(i, _)) => bind(*i, other, subst),
        (TcType::Int, TcType::Int)
        | (TcType::Float, TcType::Float)
        | (TcType::Bool, TcType::Bool)
        | (TcType::String, TcType::String)
        | (TcType::Unit, TcType::Unit) => Ok(()),
        (TcType::List(x), TcType::List(y)) => unify_into(x, y, subst),
        (TcType::Option(x), TcType::Option(y)) => unify_into(x, y, subst),
        (TcType::Map(kx, vx), TcType::Map(ky, vy)) => {
            unify_into(kx, ky, subst)?;
            unify_into(vx, vy, subst)
        }
        (TcType::Fn(px, rx), TcType::Fn(py, ry)) => {
            if px.len() != py.len() {
                return Err(UnifyError::Mismatch(a, b));
            }
            for (p, q) in px.iter().zip(py) {
                unify_into(p, q, subst)?;
            }
            unify_into(rx, ry, subst)
        }
        (TcType::Record(fx), TcType::Record(fy)) => {
            unify_records(fx, fy, subst).map_err(|_| UnifyError::Mismatch(a, b))
        }
        (TcType::Variant(na, pa), TcType::Variant(nb, pb)) => {
            if na != nb {
                return Err(UnifyError::Mismatch(a, b));
            }
            unify_lists(pa, pb, subst)
        }
        (TcType::Named(na, aa), TcType::Named(nb, ab)) => {
            if na != nb {
                return Err(UnifyError::Mismatch(a, b));
            }
            unify_lists(aa, ab, subst)
        }
        // A variant value is also a value of its ADT's named type.
        (TcType::Variant(na, _), TcType::Named(nb, _))
        | (TcType::Named(nb, _), TcType::Variant(na, _)) => {
            if na == nb {
                Ok(())
            } else {
                Err(UnifyError::Mismatch(a, b))
            }
        }
        _ => Err(UnifyError::Mismatch(a, b)),
    }
}

fn unify_records(
    fx: &[(String, Box<TcType>)],
    fy: &[(String, Box<TcType>)],
    subst: &mut HashMap<u32, TcType>,
) -> Result<(), ()> {
    if fx.len() != fy.len() {
        return Err(());
    }
    for (nx, tx) in fx {
        let Some((_, ty)) = fy.iter().find(|(ny, _)| ny == nx) else {
            return Err(());
        };
        unify_into(tx, ty, subst).map_err(|_| ())?;
    }
    Ok(())
}

fn unify_lists(
    xs: &[TcType],
    ys: &[TcType],
    subst: &mut HashMap<u32, TcType>,
) -> Result<(), UnifyError> {
    if xs.len() != ys.len() {
        return Err(UnifyError::Mismatch(TcType::Unit, TcType::Unit));
    }
    for (x, y) in xs.iter().zip(ys) {
        unify_into(x, y, subst)?;
    }
    Ok(())
}

fn bind(id: u32, ty: &TcType, subst: &mut HashMap<u32, TcType>) -> Result<(), UnifyError> {
    if let TcType::Var(other) = ty {
        if *other == id {
            return Ok(());
        }
    }
    if ty.free_vars().contains(&id) {
        return Err(UnifyError::Recursive(ty.clone()));
    }
    subst.insert(id, ty.clone());
    Ok(())
}

/// Convenience: unify two types and return the completed substitution.
///
/// # Errors
///
/// Returns [`UnifyError`] when the types are incompatible.
pub fn unify(a: &TcType, b: &TcType) -> Result<HashMap<u32, TcType>, UnifyError> {
    let mut subst = HashMap::new();
    unify_into(a, b, &mut subst)?;
    Ok(subst)
}
