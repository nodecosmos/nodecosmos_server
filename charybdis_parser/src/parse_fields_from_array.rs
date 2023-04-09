use syn::{Expr, ExprArray};
use crate::parse_string_literal;


pub fn parse_fields_from_array_ref(array_ref_expr: &Expr) -> Vec<String> {
    match array_ref_expr {
        Expr::Reference(reference) => match *reference.expr.clone() {
            Expr::Array(array) => parse_array_expr(array),
            _ => panic!("Expected an array expression"),
        },
        _ => panic!("Expected an address-of expression"),
    }
}

pub fn parse_array_expr(array_expr: ExprArray) -> Vec<String> {
   array_expr.elems.into_iter()
       .filter_map(|expr| parse_string_literal(expr))
       .collect()
}
