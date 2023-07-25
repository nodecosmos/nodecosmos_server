use charybdis::Deserialize;

#[derive(Deserialize)]
pub enum EditContext {
    DirectEdit,
    ContributionRequest,
}

#[derive(Deserialize)]
pub struct NodeContextParams {
    #[serde(default = "direct_edit")]
    pub context: EditContext,
}

pub fn direct_edit() -> EditContext {
    EditContext::DirectEdit
}
