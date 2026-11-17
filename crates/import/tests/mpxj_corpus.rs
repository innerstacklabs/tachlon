use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

fn xml_files(root: &Path) -> Vec<PathBuf> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", directory.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "could not read an entry in {}: {error}",
                    directory.display()
                )
            });
            let file_type = entry.file_type().unwrap_or_else(|error| {
                panic!("could not inspect {}: {error}", entry.path().display())
            });
            if file_type.is_dir() {
                directories.push(entry.path());
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
            {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

#[test]
fn parses_mpxj_junit_data() {
    let Some(root) = std::env::var_os("MPXJ_DATA").map(PathBuf::from) else {
        eprintln!("MPXJ_DATA not set, skipping");
        return;
    };
    if !root.is_dir() {
        eprintln!("MPXJ_DATA not set, skipping");
        return;
    }

    let mut parsed = 0_usize;
    let mut import_error = 0_usize;
    let mut not_utf8 = 0_usize;
    let mut not_mspdi = 0_usize;
    let mut errors = Vec::new();
    let files = xml_files(&root);

    for path in &files {
        let bytes = fs::read(path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        let Ok(text) = std::str::from_utf8(&bytes) else {
            not_utf8 += 1;
            continue;
        };
        if !text.contains("schemas.microsoft.com/project") {
            not_mspdi += 1;
            continue;
        }
        #[expect(
            clippy::match_wild_err_arm,
            reason = "a panic payload has nothing to print; the message names the file"
        )]
        match catch_unwind(AssertUnwindSafe(|| import::parse_mspdi(text))) {
            Ok(Ok(_)) => parsed += 1,
            Ok(Err(error)) => {
                import_error += 1;
                errors.push(format!("{}: {error}", path.display()));
            }
            Err(_) => panic!("parser panicked for {}", path.display()),
        }
    }

    eprintln!(
        "parsed: {parsed}, import_error: {import_error}, not_utf8: {not_utf8}, \
         not_mspdi: {not_mspdi}, total: {}",
        files.len()
    );
    for error in errors.iter().take(20) {
        eprintln!("{error}");
    }
}
