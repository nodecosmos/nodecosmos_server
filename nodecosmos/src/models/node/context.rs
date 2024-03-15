#[derive(PartialEq, Default, Clone, Copy)]
pub enum Context {
    #[default]
    None,
    Merge,
    MergeRecovery,
    BranchedInit,
}
