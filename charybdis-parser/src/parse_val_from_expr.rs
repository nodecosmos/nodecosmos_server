use syn::Expr;

#[allow(dead_code)]
pub fn parse_val_from_expr(array_expr: &Expr) -> String {
    if let Expr::Lit(lit) = array_expr {
        if let syn::Lit::Str(s) = &lit.lit {
            s.value()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}
