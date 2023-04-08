pub(crate) fn filter_non_empty_keys(keys: Vec<String>) -> Vec<String> {
    let res: Vec<String> = keys.iter()
        .filter(|key| !key.is_empty())
        .map(|k| k.to_string())
        .collect::<Vec<String>>();
    res
}
