//! Checks that a LaTeX template defines every macro the export emits.
//!
//! Uses only `std`: this file is also included by `build.rs`, so stored
//! templates are checked on every `cargo build`.

/// Commands used by the generated `.tex` (see `export.rs`).
pub const REQUIRED_COMMANDS: &[&str] = &[
    "Kapitel",
    "RezeptInfo",
    "Beschreibung",
    "Zutat",
    "OptionaleZutat",
    "Schritt",
    "OptionalerSchritt",
];

/// Environments used by the generated `.tex` (see `export.rs`).
pub const REQUIRED_ENVIRONMENTS: &[&str] = &["Rezept", "Zutaten", "Zutatengruppe", "Zubereitung"];

const COMMAND_DEFINERS: &[&str] = &[
    "newcommand",
    "renewcommand",
    "providecommand",
    "DeclareRobustCommand",
    "NewDocumentCommand",
    "RenewDocumentCommand",
    "ProvideDocumentCommand",
    "DeclareDocumentCommand",
    "def",
    "gdef",
    "edef",
    "xdef",
];

const ENVIRONMENT_DEFINERS: &[&str] = &[
    "newenvironment",
    "renewenvironment",
    "NewDocumentEnvironment",
    "RenewDocumentEnvironment",
    "ProvideDocumentEnvironment",
    "DeclareDocumentEnvironment",
];

/// Returns `Err` with a readable list of missing definitions.
pub fn check_template(style: &str) -> Result<(), Vec<String>> {
    let code = strip_comments(style);
    let mut missing = Vec::new();

    for command in REQUIRED_COMMANDS {
        if !defines_command(&code, command) {
            missing.push(format!("command \\{command}"));
        }
    }
    for environment in REQUIRED_ENVIRONMENTS {
        if !defines_environment(&code, environment) {
            missing.push(format!("environment {environment}"));
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Removes `%` comments (but keeps escaped `\%`).
fn strip_comments(style: &str) -> String {
    style
        .lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let end = (0..bytes.len())
                .find(|&index| bytes[index] == b'%' && (index == 0 || bytes[index - 1] != b'\\'))
                .unwrap_or(bytes.len());
            &line[..end]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Matches `\definer{\name}`, `\definer\name` and `\definer*{\name}` without
/// accepting longer names such as `\Zutatengruppe` for `\Zutat`.
fn defines_command(code: &str, name: &str) -> bool {
    COMMAND_DEFINERS.iter().any(|definer| {
        let prefix = format!("\\{definer}");
        let target = format!("\\{name}");
        code.match_indices(&prefix).any(|(index, _)| {
            let rest = &code[index + prefix.len()..];
            if rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
                return false; // a longer definer, e.g. \definecolor for \def
            }
            let rest = rest.strip_prefix('*').unwrap_or(rest).trim_start();
            let rest = rest.strip_prefix('{').map(str::trim_start).unwrap_or(rest);
            rest.strip_prefix(&target)
                .is_some_and(|after| !after.starts_with(|c: char| c.is_ascii_alphabetic()))
        })
    })
}

fn defines_environment(code: &str, name: &str) -> bool {
    ENVIRONMENT_DEFINERS.iter().any(|definer| {
        let prefix = format!("\\{definer}");
        code.match_indices(&prefix).any(|(index, _)| {
            let rest = code[index + prefix.len()..].trim_start();
            let rest = rest.strip_prefix('*').unwrap_or(rest).trim_start();
            rest.strip_prefix('{')
                .and_then(|rest| rest.trim_start().strip_prefix(name))
                .is_some_and(|after| after.trim_start().starts_with('}'))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_template() -> String {
        let mut style = String::new();
        for command in REQUIRED_COMMANDS {
            style.push_str(&format!("\\newcommand{{\\{command}}}[1]{{#1}}\n"));
        }
        for environment in REQUIRED_ENVIRONMENTS {
            style.push_str(&format!("\\newenvironment{{{environment}}}{{}}{{}}\n"));
        }
        style
    }

    #[test]
    fn complete_template_passes() {
        assert_eq!(check_template(&complete_template()), Ok(()));
    }

    #[test]
    fn missing_definitions_are_listed() {
        let style = complete_template()
            .replace("\\newcommand{\\Schritt}", "\\newcommand{\\Step}")
            .replace("\\newenvironment{Zutaten}", "\\newenvironment{Ingredients}");
        assert_eq!(
            check_template(&style),
            Err(vec!["command \\Schritt".to_string(), "environment Zutaten".to_string()])
        );
    }

    #[test]
    fn longer_names_do_not_count() {
        let style = complete_template().replace("\\newcommand{\\Zutat}", "\\newcommand{\\ZutatX}");
        assert_eq!(check_template(&style), Err(vec!["command \\Zutat".to_string()]));
    }

    #[test]
    fn commented_out_definitions_do_not_count() {
        let style = complete_template().replace("\\newcommand{\\Kapitel}", "% \\newcommand{\\Kapitel}");
        assert_eq!(check_template(&style), Err(vec!["command \\Kapitel".to_string()]));
    }

    #[test]
    fn alternative_definition_forms_are_accepted() {
        assert!(defines_command("\\def\\Zutat#1#2{}", "Zutat"));
        assert!(defines_command("\\NewDocumentCommand{\\Zutat}{mm}{}", "Zutat"));
        assert!(defines_command("\\newcommand*\\Zutat[2]{}", "Zutat"));
        assert!(defines_command("\\renewcommand { \\Zutat }{}", "Zutat"));
        assert!(!defines_command("\\definecolor\\Zutat", "Zutat"));
        assert!(defines_environment("\\NewDocumentEnvironment{Rezept}{mm}{}{}", "Rezept"));
        assert!(defines_environment("\\newenvironment{ Rezept }{}{}", "Rezept"));
        assert!(!defines_environment("\\newenvironment{Rezepte}{}{}", "Rezept"));
    }

    #[test]
    fn escaped_percent_is_not_a_comment() {
        assert_eq!(strip_comments("50\\% done % note"), "50\\% done ");
    }
}
