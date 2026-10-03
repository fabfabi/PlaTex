//! Fails the build if a stored template in `templates/` misses a required definition.

#[allow(dead_code)]
#[path = "src/template_check.rs"]
mod template_check;

use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=templates");
    println!("cargo:rerun-if-changed=src/template_check.rs");

    let mut errors = Vec::new();
    for entry in fs::read_dir("templates").expect("templates/ directory is missing") {
        let path = entry.expect("cannot read templates/").path();
        if path.extension().is_some_and(|extension| extension == "sty") {
            let style = fs::read_to_string(&path).expect("cannot read template");
            if let Err(missing) = template_check::check_template(&style) {
                errors.push(format!(
                    "{}: missing {}",
                    path.display(),
                    missing.join(", ")
                ));
            }
        }
    }

    if !errors.is_empty() {
        panic!("Invalid LaTeX templates:\n{}", errors.join("\n"));
    }
}
