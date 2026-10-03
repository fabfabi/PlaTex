use std::{
    fmt::Write,
    fs, io,
    path::{Path, PathBuf},
};

use crate::models::{Ingredient, RecipeDetail, Tag, TagKind};

/// A layout built into the executable. Every template must pass
/// `template_check::check_template` (enforced by `build.rs` and tests).
pub struct Template {
    pub name: &'static str,
    pub style: &'static str,
}

pub const TEMPLATES: &[Template] = &[Template {
    name: "A5 vertical split",
    style: include_str!("../templates/a5_vertical_split.sty"),
}];

/// File name under which the chosen template is saved next to the `.tex`.
pub const STYLE_FILE_NAME: &str = "platex.sty";

/// Writes the cookbook to `tex_path` and the template `style` as `platex.sty` beside it.
pub fn write_cookbook(tex_path: &Path, style: &str, recipes: &[RecipeDetail]) -> io::Result<PathBuf> {
    let style_path = tex_path.with_file_name(STYLE_FILE_NAME);
    fs::write(tex_path, render_cookbook(recipes))?;
    fs::write(&style_path, style)?;
    Ok(style_path)
}

/// Escapes characters that have a special meaning in LaTeX (with ngerman babel).
pub fn escape_latex(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => escaped.push_str(r"\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                escaped.push('\\');
                escaped.push(c);
            }
            '~' => escaped.push_str(r"\textasciitilde{}"),
            '^' => escaped.push_str(r"\textasciicircum{}"),
            '"' => escaped.push_str(r"\textquotedbl{}"),
            '\r' => {}
            _ => escaped.push(c),
        }
    }
    escaped
}

/// File-name friendly key of a content tag, used to look up `icons/<key>.png`.
fn icon_key(tag_name: &str) -> String {
    tag_name
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

fn quantity_text(ingredient: &Ingredient) -> String {
    match (&ingredient.quantity, &ingredient.unit) {
        (Some(quantity), Some(unit)) => format!("{} {}", quantity, unit),
        (Some(quantity), None) => quantity.clone(),
        (None, Some(unit)) => unit.clone(),
        (None, None) => String::new(),
    }
}

fn optional_number(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

/// Renders one recipe as a `Rezept` environment (see `templates/platex.sty`).
pub fn render_recipe(detail: &RecipeDetail) -> String {
    let recipe = &detail.recipe;
    let icon = detail
        .tags
        .iter()
        .find(|tag| tag.kind == TagKind::Content)
        .map(|tag| icon_key(&tag.name))
        .unwrap_or_default();

    let mut out = String::new();
    writeln!(out, r"\begin{{Rezept}}{{{}}}{{{}}}", escape_latex(&recipe.name), icon).unwrap();

    if recipe.servings.is_some() || recipe.prep_time_minutes.is_some() || recipe.cook_time_minutes.is_some() {
        writeln!(
            out,
            r"  \RezeptInfo{{{}}}{{{}}}{{{}}}",
            optional_number(recipe.servings),
            optional_number(recipe.prep_time_minutes),
            optional_number(recipe.cook_time_minutes),
        )
        .unwrap();
    }

    if let Some(description) = recipe.description.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        writeln!(out, r"  \Beschreibung{{{}}}", escape_latex(description)).unwrap();
    }

    // No blank lines between both columns: a paragraph break would stack them.
    if !detail.ingredients.is_empty() {
        writeln!(out, r"  \begin{{Zutaten}}").unwrap();
        let mut current_group: Option<Option<&str>> = None;
        for ingredient in &detail.ingredients {
            let group = ingredient.group_name.as_deref();
            if current_group != Some(group) {
                if current_group.is_some() {
                    writeln!(out, r"    \end{{Zutatengruppe}}").unwrap();
                }
                writeln!(out, r"    \begin{{Zutatengruppe}}{{{}}}", escape_latex(group.unwrap_or_default())).unwrap();
                current_group = Some(group);
            }
            let command = if ingredient.optional { "OptionaleZutat" } else { "Zutat" };
            writeln!(
                out,
                r"      \{}{{{}}}{{{}}}",
                command,
                escape_latex(&quantity_text(ingredient)),
                escape_latex(&ingredient.ingredient_name),
            )
            .unwrap();
        }
        writeln!(out, r"    \end{{Zutatengruppe}}").unwrap();
        writeln!(out, r"  \end{{Zutaten}}").unwrap();
    }

    if !detail.steps.is_empty() {
        writeln!(out, r"  \begin{{Zubereitung}}").unwrap();
        for step in &detail.steps {
            let command = if step.optional { "OptionalerSchritt" } else { "Schritt" };
            writeln!(out, r"    \{}{{{}}}", command, escape_latex(&step.instruction)).unwrap();
        }
        writeln!(out, r"  \end{{Zubereitung}}").unwrap();
    }

    writeln!(out, r"\end{{Rezept}}").unwrap();
    out
}

/// Chapter at the end of the cookbook holding all recipes without a chapter.
pub const OTHER_CHAPTER_NAME: &str = "Sonstige";

/// Renders a complete cookbook: each chapter (by chapter order) with its
/// recipes, then recipes without a chapter under `OTHER_CHAPTER_NAME`.
/// Recipes are sorted by name; a recipe with several chapters appears in each.
pub fn render_cookbook(recipes: &[RecipeDetail]) -> String {
    let mut sorted = recipes.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|detail| detail.recipe.name.to_lowercase());

    let mut chapters = recipes
        .iter()
        .flat_map(|detail| detail.tags.iter())
        .filter(|tag| tag.kind == TagKind::Chapter)
        .collect::<Vec<&Tag>>();
    chapters.sort_by(|a, b| (a.sort_order, &a.name, &a.id).cmp(&(b.sort_order, &b.name, &b.id)));
    chapters.dedup_by(|a, b| a.id == b.id);

    let has_chapter = |detail: &RecipeDetail, chapter_id: Option<&str>| {
        detail
            .tags
            .iter()
            .any(|tag| tag.kind == TagKind::Chapter && chapter_id.map_or(true, |id| tag.id == id))
    };

    let mut out = String::new();
    writeln!(out, r"\documentclass[a5paper]{{article}}").unwrap();
    writeln!(out, r"\usepackage{{platex}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, r"\begin{{document}}").unwrap();

    for chapter in chapters {
        writeln!(out).unwrap();
        writeln!(out, r"\Kapitel{{{}}}", escape_latex(&chapter.name)).unwrap();
        for detail in sorted.iter().filter(|detail| has_chapter(detail, Some(&chapter.id))) {
            writeln!(out).unwrap();
            out.push_str(&render_recipe(detail));
        }
    }

    let without_chapter = sorted.iter().filter(|detail| !has_chapter(detail, None)).collect::<Vec<_>>();
    if !without_chapter.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, r"\Kapitel{{{}}}", OTHER_CHAPTER_NAME).unwrap();
        for detail in without_chapter {
            writeln!(out).unwrap();
            out.push_str(&render_recipe(detail));
        }
    }

    writeln!(out).unwrap();
    writeln!(out, r"\end{{document}}").unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Recipe, Step};

    fn tag(id: &str, name: &str, kind: TagKind, sort_order: i64) -> Tag {
        Tag {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            sort_order,
        }
    }

    fn ingredient(group: Option<&str>, quantity: Option<&str>, unit: Option<&str>, name: &str, optional: bool) -> Ingredient {
        Ingredient {
            id: 0,
            recipe_id: "r".to_string(),
            group_name: group.map(str::to_string),
            quantity: quantity.map(str::to_string),
            unit: unit.map(str::to_string),
            ingredient_name: name.to_string(),
            optional,
            sort_order: 0,
        }
    }

    fn step(instruction: &str, optional: bool) -> Step {
        Step {
            id: 0,
            recipe_id: "r".to_string(),
            step_number: 0,
            instruction: instruction.to_string(),
            optional,
            sort_order: 0,
        }
    }

    fn recipe(name: &str, tags: Vec<Tag>) -> RecipeDetail {
        RecipeDetail {
            recipe: Recipe {
                id: name.to_string(),
                name: name.to_string(),
                description: None,
                servings: None,
                prep_time_minutes: None,
                cook_time_minutes: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            ingredients: vec![],
            steps: vec![],
            tags,
        }
    }

    #[test]
    fn escape_latex_escapes_special_characters() {
        assert_eq!(
            escape_latex(r#"50% & $5 #1 a_b {x} ~ ^ \ "q""#),
            r"50\% \& \$5 \#1 a\_b \{x\} \textasciitilde{} \textasciicircum{} \textbackslash{} \textquotedbl{}q\textquotedbl{}"
        );
        assert_eq!(escape_latex("Gemüse\r\n"), "Gemüse\n");
    }

    #[test]
    fn render_recipe_fills_all_sections() {
        let mut detail = recipe(
            "Pasta & Sauce",
            vec![tag("c1", "Mains", TagKind::Chapter, 0), tag("t1", "Vegan", TagKind::Content, 0)],
        );
        detail.recipe.description = Some("Quick".to_string());
        detail.recipe.servings = Some(2);
        detail.recipe.cook_time_minutes = Some(15);
        detail.ingredients = vec![
            ingredient(Some("Base"), Some("2"), Some("EL"), "Olivenöl", false),
            ingredient(Some("Base"), None, None, "Salz", true),
            ingredient(Some("Gewürze"), Some("1"), Some("TL"), "Oregano", false),
        ];
        detail.steps = vec![step("Kochen.", false), step("Garnieren.", true)];

        let expected = r"\begin{Rezept}{Pasta \& Sauce}{vegan}
  \RezeptInfo{2}{}{15}
  \Beschreibung{Quick}
  \begin{Zutaten}
    \begin{Zutatengruppe}{Base}
      \Zutat{2 EL}{Olivenöl}
      \OptionaleZutat{}{Salz}
    \end{Zutatengruppe}
    \begin{Zutatengruppe}{Gewürze}
      \Zutat{1 TL}{Oregano}
    \end{Zutatengruppe}
  \end{Zutaten}
  \begin{Zubereitung}
    \Schritt{Kochen.}
    \OptionalerSchritt{Garnieren.}
  \end{Zubereitung}
\end{Rezept}
";
        assert_eq!(render_recipe(&detail), expected);
    }

    #[test]
    fn render_recipe_omits_empty_sections() {
        let rendered = render_recipe(&recipe("Toast", vec![]));
        assert_eq!(rendered, "\\begin{Rezept}{Toast}{}\n\\end{Rezept}\n");
    }

    #[test]
    fn render_cookbook_orders_chapters_and_recipes() {
        let soups = tag("c1", "Suppen", TagKind::Chapter, 1);
        let mains = tag("c2", "Hauptgerichte", TagKind::Chapter, 0);
        let recipes = vec![
            recipe("Tomatensuppe", vec![soups.clone()]),
            recipe("Brot", vec![]),
            recipe("Eintopf", vec![soups.clone(), mains.clone()]),
            recipe("Auflauf", vec![mains.clone()]),
        ];

        let rendered = render_cookbook(&recipes);
        let order = [
            r"\begin{document}",
            r"\Kapitel{Hauptgerichte}",
            r"\begin{Rezept}{Auflauf}",
            r"\begin{Rezept}{Eintopf}",
            r"\Kapitel{Suppen}",
            r"\begin{Rezept}{Eintopf}",
            r"\begin{Rezept}{Tomatensuppe}",
            r"\Kapitel{Sonstige}",
            r"\begin{Rezept}{Brot}",
            r"\end{document}",
        ];
        let mut position = 0;
        for needle in order {
            let found = rendered[position..].find(needle).unwrap_or_else(|| panic!("missing {needle}"));
            position += found + needle.len();
        }
        assert_eq!(rendered.matches(r"\Kapitel{").count(), 3);
        assert!(rendered.starts_with("\\documentclass[a5paper]{article}\n\\usepackage{platex}\n"));
    }

    #[test]
    fn render_cookbook_omits_other_chapter_when_all_recipes_have_chapters() {
        let rendered = render_cookbook(&[recipe("Suppe", vec![tag("c1", "Suppen", TagKind::Chapter, 0)])]);
        assert!(!rendered.contains(r"\Kapitel{Sonstige}"));
    }

    #[test]
    fn write_cookbook_writes_tex_and_style() {
        let dir = std::env::temp_dir().join(format!("platex_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let tex_path = dir.join("book.tex");

        let style_path = write_cookbook(&tex_path, TEMPLATES[0].style, &[recipe("Brot", vec![])]).unwrap();

        assert_eq!(style_path, dir.join("platex.sty"));
        assert!(fs::read_to_string(&tex_path).unwrap().contains(r"\begin{Rezept}{Brot}"));
        assert_eq!(fs::read_to_string(&style_path).unwrap(), TEMPLATES[0].style);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_built_in_template_passes_check() {
        for template in TEMPLATES {
            assert_eq!(crate::template_check::check_template(template.style), Ok(()), "{}", template.name);
        }
    }

    #[test]
    fn check_covers_every_macro_the_export_emits() {
        let mut detail = recipe("Suppe", vec![tag("c1", "Suppen", TagKind::Chapter, 0)]);
        detail.recipe.servings = Some(2);
        detail.recipe.description = Some("Warm".to_string());
        detail.ingredients = vec![
            ingredient(Some("Base"), Some("1"), None, "Wasser", false),
            ingredient(None, None, None, "Salz", true),
        ];
        detail.steps = vec![step("Kochen.", false), step("Salzen.", true)];
        let rendered = render_cookbook(&[detail]);

        let standard = ["documentclass", "usepackage", "begin", "end"];
        for token in rendered.split('\\').skip(1) {
            let name = token.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>();
            if name.is_empty() || standard.contains(&name.as_str()) {
                continue;
            }
            assert!(crate::template_check::REQUIRED_COMMANDS.contains(&name.as_str()), "\\{name} is not checked");
        }
        for token in rendered.split(r"\begin{").skip(1) {
            let name = &token[..token.find('}').unwrap()];
            if name != "document" {
                assert!(crate::template_check::REQUIRED_ENVIRONMENTS.contains(&name), "{name} is not checked");
            }
        }
    }
}
