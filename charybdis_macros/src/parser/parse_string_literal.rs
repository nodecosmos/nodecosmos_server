use syn::{Expr, Lit};

pub fn parse_string_literal(expr: Expr) -> Option<String> {
    match expr {
        Expr::Lit(lit) => match lit.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}
