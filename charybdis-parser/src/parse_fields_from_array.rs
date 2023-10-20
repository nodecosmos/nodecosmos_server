use crate::{parse_string_literal, LocalIndexTarget};
use quote::ToTokens;
use syn::{Expr, ExprArray, ExprTuple};

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
        .filter_map(|elem| {
            if let Expr::Tuple(ExprTuple { elems, .. }) = elem {
                let elems: Vec<_> = elems.iter().collect();

                if elems.len() != 2 {
                    return None;
                }

                let pk = if let Expr::Array(ref array) = *elems[0] {
                    array
                        .elems
                        .iter()
                        .filter_map(|e| {
                            if let Expr::Path(path) = e {
                                Some(path.path.segments.last()?.ident.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    return None;
                };

                let ck = if let Expr::Array(ref array) = *elems[1] {
                    array
                        .elems
                        .iter()
                        .filter_map(|e| {
                            if let Expr::Path(path) = e {
                                Some(path.path.segments.last()?.ident.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    return None;
                };

                Some(LocalIndexTarget { pk, ck })
            } else {
                None
            }
        })
        .collect()
}
