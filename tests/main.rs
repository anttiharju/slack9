use assert_cmd::cargo::cargo_bin_cmd;

fn run() -> (String, String) {
    let output = cargo_bin_cmd!("rust-starter").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr)
}

#[test]
fn test_changes_output() {
    let (stdout, stderr) = run();

    for expected in ["Hello world!"] {
        assert!(stdout.contains(expected), "Expected output to contain '{}'", expected);
        assert!(!stderr.contains(expected), "Did not expect '{}' in stderr", expected);
    }
}

#[test]
fn test_exitcode_usage() {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    // Extract exitcode function names from the macro definition
    let exitcode_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/exitcode/mod.rs");
    let exitcode_content = fs::read_to_string(&exitcode_file).unwrap();

    let mut exitcodes = Vec::new();
    let mut in_macro = false;
    for line in exitcode_content.lines() {
        if line.contains("define_exitcodes!") && line.contains("{") {
            in_macro = true;
            continue;
        }
        if in_macro && line.trim() == "}" {
            break;
        }
        if in_macro {
            if let Some(pos) = line.find("=>") {
                let fn_name = line[..pos].trim().trim_end_matches(',');
                if !fn_name.is_empty() {
                    exitcodes.push(fn_name.to_string());
                }
            }
        }
    }

    // Count usage of each exitcode function in src/
    let mut counts: HashMap<String, usize> = HashMap::new();
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    fn visit_dir(dir: &Path, counts: &mut HashMap<String, usize>, exitcodes: &[String]) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dir(&path, counts, exitcodes);
                } else if path.extension().map_or(false, |e| e == "rs") {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        for exitcode in exitcodes {
                            let pattern = format!("exitcode::{}(", exitcode);
                            *counts.entry(exitcode.clone()).or_insert(0) += contents.matches(&pattern).count();
                        }
                    }
                }
            }
        }
    }

    visit_dir(&src_dir, &mut counts, &exitcodes);

    // Assert each exitcode is called exactly once
    for exitcode in &exitcodes {
        let count = counts.get(exitcode).copied().unwrap_or(0);
        assert_eq!(count, 1, "expected exactly 1 call to exitcode::{}, found {}", exitcode, count);
    }
}
