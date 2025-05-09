## Macros

Rust macros are a powerful tool for metaprogramming, allowing you to write code that generates other
code during compilation. There are two main types of macros in Rust: declarative macros and
procedural macros. I'll explain the syntax and usage of each, along with the role of the # symbol
and other rules and patterns.

Declarative Macros:
Declarative macros are defined with the macro_rules! keyword. They use a pattern-matching approach
to generate code. The basic syntax is:

```rust
macro_rules! macro_name {
    (pattern) => {
    // code to generate
    };
}
```

The pattern can contain various components, such as identifiers, literals, types, and other
patterns. Here are the most common pattern types:

- $ident:ident: Matches an identifier (variable or function name).
- $lit:literal: Matches a literal value (e.g., number, string, or character).
- $ty:ty: Matches a type (e.g., i32, String, or custom types).
- $expr:expr: Matches an expression (e.g., a + b, foo(), or 42).

Additionally, you can use the following syntax to repeat patterns:

- $(...),*: Matches zero or more repetitions of the enclosed pattern, separated by commas.
- $(...);*: Matches zero or more repetitions of the enclosed pattern, separated by semicolons.

The # symbol is used in declarative macros to "splice" an expression or pattern captured by the
macro into the generated code. When you use $variable in the code block, you need to prefix it with
a # symbol to tell Rust that it's a captured variable, not a regular one.

Example:

```rust
macro_rules! vec {
    ($($x:expr),* $(,)?) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}

fn main() {
    let v = vec![1, 2, 3];
    println!("{:?}", v);
}
```

Procedural Macros:
Procedural macros are more powerful and complex than declarative macros. They are Rust functions
that accept input as a TokenStream, manipulate it, and return a new TokenStream. Procedural macros
come in three flavors: function-like, derive, and attribute macros.

a. Function-like macros: These macros look like regular function calls. They are defined with the
proc_macro:: attribute and return a TokenStream. Example:

```rust

use proc_macro::TokenStream;

#[proc_macro]
pub fn my_macro(input: TokenStream) -> TokenStream {
// Manipulate input and generate output
}
```

b. Derive macros: These macros automatically implement a trait for a given struct or enum. They are
defined with the #[proc_macro_derive(TraitName)] attribute. Example:

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(MyTrait)]
pub fn my_trait_derive(input: TokenStream) -> TokenStream {
let input = parse_macro_input!(input as DeriveInput);

    // Generate the implementation for MyTrait
    let output = quote! {
        // Generated code
    };

    output.into()
}
```

c. Attribute macros: These macros are used as an attribute on a function, struct, or enum, allowing
you to generate code based on the annotated item. They are defined with the
`#[proc_macro_attribute]` attribute.

Example:

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, AttributeArgs};

#[proc_macro_attribute]
pub fn my_attribute(args: TokenStream, input: TokenStream) -> TokenStream {
let _args = parse_macro_input!(args as AttributeArgs);
let input = parse_macro_input!(input as ItemFn);

    // Generate the modified function or additional code
    let output = quote! {
        // Generated code
    };

    output.into()
}
```

To use procedural macros in your project, you'll typically need the following dependencies:

proc-macro2: Provides a stable version of proc_macro with additional features.

- syn: A parsing library for Rust syntax.
- quote: A library for generating Rust code within macros.

In summary, Rust macros come in two types: declarative and procedural.
Declarative macros use the macro_rules! keyword and pattern matching, while procedural macros
include function-like,
derive, and attribute macros.

The # symbol is used in declarative macros to splice captured variables into the generated code.
Other patterns and rules vary depending on the macro type and syntax.

### Macro hygiene

If we have macros that generate another macros, we need to be careful to avoid
naming conflicts so that new macros don't accidentally shadow existing ones.
