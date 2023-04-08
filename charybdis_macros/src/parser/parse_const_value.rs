use syn::{Expr, ImplItem, ItemImpl};

pub fn parse_const_value(input: &ItemImpl, const_name: String) -> &Expr {
    input.items
        .iter()
        .find_map(|item| match item {
            ImplItem::Const(c) if c.ident == const_name => Some(&c.expr),
            _ => None,
        })
        .expect(&format!("Missing {}", const_name))
}
