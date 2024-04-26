pub async fn read_file_names(dir: &str, prefix: &str) -> Vec<String> {
    let mut files = vec![];

    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.starts_with(prefix) {
            files.push(format!("{}/{}", dir, file_name));
        }
    }

    files
}
