use crate::{parse_string_literal, LocalIndexTarget};
use quote::ToTokens;
use syn::{Expr, ExprArray};

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
    array_expr.elems.into_iter().filter_map(parse_string_literal).collect()
}

pub fn parse_arr_expr_from_literals(array_expr: ExprArray) -> Vec<String> {
    array_expr
        .elems
        .into_iter()
        .map(|elem| elem.to_token_stream().to_string())
        .collect()
}

pub fn parse_loc_sec_idx_array_expr(array_expr: ExprArray) -> Vec<LocalIndexTarget> {
    array_expr
        .elems
        .into_iter()
        .map(|elem| {
            let string = elem.to_token_stream().to_string();
            let parsed: LocalIndexTarget = serde_json::from_str(&string).unwrap();

            parsed
        })
        .collect()
}
