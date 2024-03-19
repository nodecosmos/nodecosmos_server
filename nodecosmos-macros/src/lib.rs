mod traits;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};
use traits::StructFields;

#[proc_macro_derive(Branchable)]
pub fn branchable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl crate::models::traits::Branchable for #name {
            fn id(&self) -> Uuid {
                self.id
            }

            fn branch_id(&self) -> Uuid {
                self.branch_id
            }
        }
    };

    TokenStream::from(expanded)
}

/// Branchable Derive that uses node_id as id
#[proc_macro_derive(BranchableNodeId)]
pub fn branchable_node_id_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl crate::models::traits::Branchable for #name {
            fn id(&self) -> Uuid {
                self.node_id
            }

            fn branch_id(&self) -> Uuid {
                self.branch_id
            }
        }
    };

    TokenStream::from(expanded)
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
        impl crate::models::traits::node::Parent for #name {
            async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
                if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
                    if self.is_branched() {
                        return self.branch_parent(db_session).await;
                    }
                    let parent = BaseNode::find_by_primary_key_value(&(parent_id, parent_id))
                        .execute(db_session)
                        .await?;
                    self.parent = Some(parent);
                }
                Ok(self.parent.as_mut())
            }

            async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
                if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
                    let branch_parent = BaseNode::maybe_find_by_primary_key_value(&(parent_id, self.branch_id))
                        .execute(db_session)
                        .await?;

                    match branch_parent {
                        Some(parent) => {
                            self.parent = Some(parent);
                        }
                        None => {
                            let parent = BaseNode::find_by_primary_key_value(&(parent_id, parent_id))
                                .execute(db_session)
                                .await?;
                            self.parent = Some(parent);
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
        impl crate::models::traits::authorization::AuthorizationFields for #name {
            fn is_public(&self) -> bool {
                use crate::models::traits::authorization::AuthorizationFields;

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
                use crate::models::traits::authorization::AuthorizationFields;

                if self.is_branched() {
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
                    return self.owner_id;
                }

                return match &self.auth_branch {
                    Some(branch) => Some(branch.owner_id),
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        None
                    }
                };
            }

            fn editor_ids(&self) -> Option<Set<Uuid>> {
                if self.is_original() {
                    return self.editor_ids.clone();
                }

                return match &self.auth_branch {
                    Some(branch) => branch.editor_ids.clone(),
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        None
                    }
                };
            }

        }

        impl crate::models::traits::Authorization for #name {
            async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                if self.is_original() {
                    // auth info is already initialized
                    if self.owner_id.is_some() {
                        return Ok(());
                    }

                    let auth_node = AuthNode::find_by_id_and_branch_id(self.id, self.id)
                        .execute(db_session)
                        .await?;
                    self.owner_id = auth_node.owner_id;
                    self.editor_ids = auth_node.editor_ids;
                } else {
                    // authorize by branch
                    let branch = AuthBranch::find_by_id(self.branch_id).execute(db_session).await?;
                    self.auth_branch = Some(branch);
                }

                Ok(())
            }

            async fn auth_creation(&mut self, data: &crate::api::data::RequestData) -> Result<(), NodecosmosError> {
                use crate::models::traits::node::Parent;

                if self.id != Uuid::default() {
                    return Err(NodecosmosError::Unauthorized(serde_json::json!({
                        "error": "Bad Request!",
                        "message": "Cannot create node with id!"
                    })));
                }

                if self.is_branched() {
                    self.auth_update(data).await?;
                } else if let Some(parent_id) = self.parent_id {
                    let mut auth_parent_node = crate::models::node::AuthNode::find_by_id_and_branch_id(parent_id, parent_id)
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
    let has_root_id = input.struct_fields().iter().any(|field| {
        return match field.ident {
            Some(ref ident) => ident == "root_id",
            None => false,
        };
    });

    if !has_root_id {
        return TokenStream::new();
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
