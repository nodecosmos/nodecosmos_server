mod traits;

use darling::{FromDeriveInput, FromField};
use log::warn;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident};
use traits::StructFields;

#[derive(Clone, FromField)]
#[darling(attributes(branch))]
struct BranchField {
    ident: Option<Ident>,

    #[darling(default)]
    original_id: bool,
}

#[derive(FromDeriveInput)]
#[darling(attributes(branch))]
struct BranchStruct {
    ident: Ident,

    data: darling::ast::Data<(), BranchField>,
}

#[proc_macro_derive(Branchable, attributes(branch))]
pub fn branchable_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let parsed_struct = match BranchStruct::from_derive_input(&ast) {
        Ok(value) => value,
        Err(e) => return e.write_errors().into(),
    };
    let struct_name = parsed_struct.ident;

    // Iterate over fields to find the one with `original_id`
    let original_id_fields = match parsed_struct.data {
        darling::ast::Data::Struct(fields) => fields
            .fields
            .into_iter()
            .filter(|field| field.original_id)
            .collect::<Vec<BranchField>>(),
        _ => Vec::new(),
    };

    // Ensure there's only one `original_id`
    if original_id_fields.len() == 1 {
        let field_name = original_id_fields[0]
            .ident
            .as_ref()
            .expect("Field without an identifier");

        // Generate your desired code with the unique `original_id` field
        let expanded = quote! {
            impl crate::models::traits::Branchable for #struct_name {
                fn original_id(&self) -> Uuid {
                    self.#field_name
                }

                fn branch_id(&self) -> Uuid {
                    self.branch_id
                }

                fn set_original_id(&mut self) {
                    self.branch_id = self.original_id();
                }
            }
        };

        TokenStream::from(expanded)
    } else {
        panic!("Branchable requires single #[branch(original_id)] attribute to determine `original_id`");
    }
}

/// Note: all derives implemented bellow `charybdis_model` will be automatically implemented for all partial models.
/// So by implementing `NodeAuthorization` derive for `Node` model, it will be automatically implemented for
/// `UpdateTitleNode`, `UpdateDescriptionNode`, etc.
#[proc_macro_derive(NodeParent)]
pub fn node_parent_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let has_parent_field = input.struct_fields().iter().any(|field| {
        return match field.ident {
            Some(ref ident) => ident == "parent",
            None => false,
        };
    });

    if !has_parent_field {
        return TokenStream::new();
    }

    let expanded = quote! {
        impl crate::models::traits::Parent for #name {
            async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut Box<Node>>, NodecosmosError> {
                if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
                    if self.is_branch() {
                        return self.branch_parent(db_session).await;
                    }
                    let parent = Node::find_by_branch_id_and_id(self.branch_id, parent_id)
                        .execute(db_session)
                        .await?;
                    self.parent = Some((Box::new(parent)));
                }
                Ok(self.parent.as_mut())
            }

            async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut Box<Node>>, NodecosmosError> {
                if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
                    let branch_parent = Node::maybe_find_first_by_branch_id_and_id(self.branch_id, parent_id)
                        .execute(db_session)
                        .await?;

                    match branch_parent {
                        Some(parent) => {
                            self.parent = Some((Box::new(parent)));
                        }
                        None => {
                            let mut parent = Node::find_by_branch_id_and_id(self.original_id(), parent_id)
                                .execute(db_session)
                                .await?;

                            parent.branch_id = self.branch_id;

                            self.parent = Some(Box::new(parent));
                        }
                    }
                }

                Ok(self.parent.as_mut())
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(NodeAuthorization)]
pub fn authorization_node_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let has_auth_branch = input.struct_fields().iter().any(|field| {
        return match field.ident {
            Some(ref ident) => ident == "auth_branch",
            None => false,
        };
    });

    if !has_auth_branch {
        return TokenStream::new();
    }

    let expanded = quote! {
        impl crate::models::traits::AuthorizationFields for #name {
            fn is_public(&self) -> bool {
                use crate::models::traits::AuthorizationFields;

                if self.is_original() {
                    return self.is_public;
                }

                return match &self.auth_branch {
                    Some(branch) => branch.is_public,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        false
                    }
                };
            }

            fn is_frozen(&self) -> bool {
                use crate::models::traits::AuthorizationFields;

                if self.is_branch() {
                    return match &self.auth_branch {
                        Some(branch) => branch.is_frozen(),
                        None => {
                            log::error!("Branched node {} has no branch!", self.id);

                            false
                        }
                    };
                }

                false
            }

            fn owner_id(&self) -> Option<Uuid> {
                if self.is_original() {
                    return Some(self.owner_id);
                }

                return match &self.auth_branch {
                    Some(branch) => Some(branch.owner_id),
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        None
                    }
                };
            }

            fn editor_ids(&self) -> &Option<Set<Uuid>> {
                if self.is_original() {
                    return &self.editor_ids;
                }

                return match &self.auth_branch {
                    Some(branch) => &branch.editor_ids,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        &None
                    }
                };
            }

            fn viewer_ids(&self) -> &Option<Set<Uuid>> {
                if self.is_original() {
                    return &self.viewer_ids;
                }

                return match &self.auth_branch {
                    Some(branch) => &branch.viewer_ids,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        &None
                    }
                };
            }
        }

        impl crate::models::traits::Authorization for #name {
            async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                 if self.is_original() {
                    // auth info is already initialized
                    if self.owner_id != Uuid::default() {
                        return Ok(());
                    }

                    let auth_node = AuthNode::find_by_branch_id_and_id(self.branch_id, self.id)
                        .execute(db_session)
                        .await?;

                    self.is_public = auth_node.is_public;
                    self.viewer_ids = auth_node.viewer_ids;
                    self.owner_id = auth_node.owner_id;
                    self.editor_ids = auth_node.editor_ids;
                } else {
                    // authorize by branch
                    let branch = AuthBranch::find_by_id(self.branch_id).execute(db_session).await?;
                    self.auth_branch = Some(Box::new(branch));
                }

                Ok(())
            }

            async fn auth_creation(&mut self, data: &crate::api::data::RequestData) -> Result<(), NodecosmosError> {
                use crate::models::traits::Parent;

                if !data.current_user.is_confirmed {
                    return Err(NodecosmosError::Unauthorized("User is not confirmed"));
                }

                if data.current_user.is_blocked {
                    return Err(NodecosmosError::Unauthorized("User is blocked"));
                }

                if self.id != Uuid::default() {
                    return Err(NodecosmosError::Unauthorized("Cannot create node with id"));
                }

                if self.is_branch() {
                    self.auth_update(data).await?;
                } else if let Some(parent_id) = self.parent_id {
                    let mut auth_parent_node = crate::models::node::AuthNode::find_by_branch_id_and_id(self.original_id(), parent_id)
                        .execute(data.db_session())
                        .await?;

                    auth_parent_node.auth_update(data).await?;
                }

                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Id)]
pub fn id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "id",
            None => false,
        };
    });

    if id.is_none() {
        panic!("Struct must have `id` field to derive Id");
    }

    let expanded = quote! {
        impl crate::models::traits::Id for #name {
            fn id(&self) -> Uuid {
                self.id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(RootId)]
pub fn root_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let root_id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "root_id",
            None => false,
        };
    });

    if root_id.is_none() {
        panic!("Struct must have `root_id` field to derive RootId");
    }

    let expanded = quote! {
        impl crate::models::traits::RootId for #name {
            fn root_id(&self) -> Uuid {
                self.root_id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(ObjectId)]
pub fn object_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let object_id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "object_id",
            None => false,
        };
    });

    if object_id.is_none() {
        panic!("Struct must have `object_id` field to derive ObjectId");
    }

    let expanded = quote! {
        impl crate::models::traits::ObjectId for #name {
            fn object_id(&self) -> Uuid {
                self.object_id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(NodeId)]
pub fn node_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let node_id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "node_id",
            None => false,
        };
    });

    if node_id.is_none() {
        panic!("Struct must have `node_id` field to derive NodeId");
    }

    let expanded = quote! {
        impl crate::models::traits::NodeId for #name {
            fn node_id(&self) -> Uuid {
                self.node_id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(FlowId)]
pub fn pluck_flow_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "flow_id",
            None => false,
        };
    });

    if id.is_none() {
        panic!("Struct must have `flow_id` field to derive FlowId");
    }

    let expanded = quote! {
        impl crate::models::traits::FlowId for #name {
            fn flow_id(&self) -> Uuid {
                self.flow_id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(MaybeFlowId)]
pub fn maybe_flow_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "flow_id",
            None => false,
        };
    });

    if id.is_none() {
        warn!("Struct must have `flow_id` field to derive MaybeFlowId");
        return TokenStream::new();
    }

    let expanded = quote! {
        impl crate::models::traits::MaybeFlowId for #name {
            fn maybe_flow_id(&self) -> Option<Uuid> {
                self.flow_id
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(MaybeFlowStepId)]
pub fn maybe_flow_step_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id = input.struct_fields().iter().find(|field| {
        return match field.ident {
            Some(ref ident) => ident == "flow_step_id",
            None => false,
        };
    });

    if id.is_none() {
        warn!("Struct must have `flow_id` field to derive MaybeFlowStepId");
        return TokenStream::new();
    }

    let expanded = quote! {
        impl crate::models::traits::MaybeFlowStepId for #name {
            fn maybe_flow_step_id(&self) -> Option<Uuid> {
                self.flow_step_id
            }
        }
    };

    TokenStream::from(expanded)
}
