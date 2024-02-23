#[derive(PartialEq, Default, Clone)]
pub enum Context {
    #[default]
    None,
    Merge,
    MergeRecovery,
    BranchedInit,
}
