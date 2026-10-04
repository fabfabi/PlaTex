use std::{collections::HashSet, fs, sync::Arc};

use chrono::Local;
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::models::{Tag, TagKind};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IngredientItem {
    id: u64,
    quantity_unit: String,
    description: String,
    optional: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IngredientGroup {
    id: u64,
    name: String,
    ingredients: Vec<IngredientItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PreparationStep {
    id: u64,
    instruction: String,
    optional: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RecipeDraft {
    name: String,
    description: String,
    servings: String,
    prep_minutes: String,
    cook_minutes: String,
    groups: Vec<IngredientGroup>,
    steps: Vec<PreparationStep>,
    tag_ids: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TagEntry {
    tag: Tag,
    edit_name: String,
}

enum TagAction {
    Rename(String, String),
    Delete(String),
    MoveChapter(usize, i32),
}

/// A deletion waiting for the user to confirm it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingDelete {
    Recipe { id: String, name: String },
    Tag { id: String, name: String },
}

impl PendingDelete {
    fn prompt(&self) -> String {
        match self {
            PendingDelete::Recipe { name, .. } => format!("Delete recipe \"{name}\"?"),
            PendingDelete::Tag { name, .. } => {
                format!("Delete tag \"{name}\"? It will be removed from all recipes.")
            }
        }
    }
}

/// An action that would leave the current form; it waits while the recipe has unsaved changes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FormAction {
    NewRecipe,
    LoadRecipe(String),
    ExportJson,
    ExportLatex,
    ImportJson,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecipeSummary {
    id: String,
    name: String,
    prep_minutes: String,
    cook_minutes: String,
    servings: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportMode {
    OnlyAddNew,
    OverwriteChanges,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct JsonIngredient {
    group_name: Option<String>,
    quantity_unit: String,
    description: String,
    optional: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct JsonStep {
    instruction: String,
    optional: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct JsonRecipe {
    #[serde(default)]
    uuid: Option<String>,
    name: String,
    description: Option<String>,
    servings: Option<i64>,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
    ingredients: Vec<JsonIngredient>,
    steps: Vec<JsonStep>,
    /// Uuids of the recipe's tags, referring to `JsonExportDocument::tags`.
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct JsonTag {
    uuid: String,
    name: String,
    kind: TagKind,
    sort_order: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct JsonExportDocument {
    version: u32,
    exported_at: String,
    #[serde(default)]
    tags: Vec<JsonTag>,
    recipes: Vec<JsonRecipe>,
}

async fn build_export_document(pool: &sqlx::SqlitePool) -> Result<JsonExportDocument, String> {
    let tags = crate::db::list_tags(pool)
        .await
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|tag| JsonTag {
            uuid: tag.id,
            name: tag.name,
            kind: tag.kind,
            sort_order: tag.sort_order,
        })
        .collect();

    let recipes = crate::db::list_recipes(pool)
        .await
        .map_err(|err| err.to_string())?;
    let mut exported = Vec::new();

    for recipe in recipes {
        let detail = crate::db::load_recipe_detail(pool, &recipe.id)
            .await
            .map_err(|err| err.to_string())?;
        let ingredients = detail
            .ingredients
            .into_iter()
            .map(|ingredient| JsonIngredient {
                group_name: ingredient.group_name,
                quantity_unit: match (ingredient.quantity, ingredient.unit) {
                    (Some(quantity), Some(unit)) => format!("{} {}", quantity, unit),
                    (Some(quantity), None) => quantity,
                    (None, Some(unit)) => unit,
                    (None, None) => String::new(),
                },
                description: ingredient.ingredient_name,
                optional: ingredient.optional,
            })
            .collect();

        let steps = detail
            .steps
            .into_iter()
            .map(|step| JsonStep {
                instruction: step.instruction,
                optional: step.optional,
            })
            .collect();

        exported.push(JsonRecipe {
            uuid: Some(recipe.id.clone()),
            name: recipe.name,
            description: recipe.description,
            servings: recipe.servings,
            prep_time_minutes: recipe.prep_time_minutes,
            cook_time_minutes: recipe.cook_time_minutes,
            ingredients,
            steps,
            tags: detail.tags.into_iter().map(|tag| tag.id).collect(),
        });
    }

    Ok(JsonExportDocument {
        version: 2,
        exported_at: Local::now().to_rfc3339(),
        tags,
        recipes: exported,
    })
}

/// Imports tags and recipes, both matched by uuid. `OnlyAddNew` skips entries
/// whose uuid already exists; `OverwriteChanges` replaces them, including the
/// recipe's tag assignments. Returns the number of imported recipes.
async fn import_document(
    pool: &sqlx::SqlitePool,
    document: JsonExportDocument,
    mode: ImportMode,
) -> Result<usize, String> {
    let existing_tag_ids = crate::db::list_tags(pool)
        .await
        .map_err(|err| format!("Could not load existing tags: {err}"))?
        .into_iter()
        .map(|tag| tag.id)
        .collect::<HashSet<_>>();

    for tag in &document.tags {
        if tag.uuid.trim().is_empty() {
            continue;
        }
        if mode == ImportMode::OnlyAddNew && existing_tag_ids.contains(&tag.uuid) {
            continue;
        }
        crate::db::upsert_tag(pool, &tag.uuid, &tag.name, tag.kind, tag.sort_order)
            .await
            .map_err(|err| format!("Could not import tag {}: {err}", tag.name))?;
    }

    let known_tag_ids = crate::db::list_tags(pool)
        .await
        .map_err(|err| format!("Could not load tags: {err}"))?
        .into_iter()
        .map(|tag| tag.id)
        .collect::<HashSet<_>>();

    let existing_ids = crate::db::list_recipes(pool)
        .await
        .map_err(|err| format!("Could not load existing recipes: {err}"))?
        .into_iter()
        .map(|recipe| recipe.id)
        .collect::<HashSet<_>>();

    let mut imported = 0usize;
    for recipe in document.recipes {
        let recipe_uuid = recipe
            .uuid
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        if mode == ImportMode::OnlyAddNew
            && recipe_uuid.is_some_and(|uuid| existing_ids.contains(uuid))
        {
            continue;
        }

        let groups = recipe
            .ingredients
            .iter()
            .map(|ingredient| crate::db::IngredientGroupInput {
                group_name: ingredient.group_name.clone(),
                quantity_unit: ingredient.quantity_unit.clone(),
                description: ingredient.description.clone(),
                optional: ingredient.optional,
            })
            .collect::<Vec<_>>();

        let steps = recipe
            .steps
            .iter()
            .map(|step| crate::db::StepInput {
                instruction: step.instruction.clone(),
                optional: step.optional,
            })
            .collect::<Vec<_>>();

        let tag_ids = recipe
            .tags
            .iter()
            .filter(|tag_id| known_tag_ids.contains(*tag_id))
            .cloned()
            .collect::<Vec<_>>();

        let saved = crate::db::upsert_recipe_with_details(
            pool,
            recipe_uuid,
            &recipe.name,
            recipe.description.as_deref(),
            recipe.servings,
            recipe.prep_time_minutes,
            recipe.cook_time_minutes,
            &groups,
            &steps,
        )
        .await;

        if let Ok(id) = saved {
            if crate::db::set_recipe_tags(pool, &id, &tag_ids)
                .await
                .is_ok()
            {
                imported += 1;
            }
        }
    }

    Ok(imported)
}

pub struct App {
    pool: Arc<sqlx::SqlitePool>,
    selected_recipe_id: Option<String>,
    recipe: RecipeDraft,
    recipes: Vec<RecipeSummary>,
    status: String,
    next_id: u64,
    import_mode: ImportMode,
    tags: Vec<TagEntry>,
    new_tag_name: String,
    new_tag_kind: TagKind,
    show_latex_export: bool,
    template_choice: TemplateChoice,
    uploaded_template: Option<UploadedTemplate>,
    pending_delete: Option<PendingDelete>,
    /// The form as it was last loaded, saved or reset; differences mean unsaved changes.
    saved_recipe: RecipeDraft,
    pending_action: Option<FormAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplateChoice {
    BuiltIn(usize),
    Uploaded,
}

/// A user-provided `.sty` that passed `template_check::check_template`.
struct UploadedTemplate {
    file_name: String,
    style: String,
}

impl App {
    pub fn new(pool: Arc<sqlx::SqlitePool>, initial_recipes: Vec<crate::models::Recipe>) -> Self {
        let mut app = Self {
            pool,
            selected_recipe_id: None,
            recipe: RecipeDraft::default(),
            recipes: Vec::new(),
            status: "Ready".to_string(),
            next_id: 1,
            import_mode: ImportMode::OverwriteChanges,
            tags: Vec::new(),
            new_tag_name: String::new(),
            new_tag_kind: TagKind::Content,
            show_latex_export: false,
            template_choice: TemplateChoice::BuiltIn(0),
            uploaded_template: None,
            pending_delete: None,
            saved_recipe: RecipeDraft::default(),
            pending_action: None,
        };
        app.refresh_tags();

        app.recipe.groups.push(IngredientGroup {
            id: app.next_id,
            name: "Base".to_string(),
            ingredients: vec![IngredientItem {
                id: app.next_id + 1,
                ..Default::default()
            }],
        });
        app.next_id += 2;
        app.recipe.steps.push(PreparationStep {
            id: app.next_id,
            ..Default::default()
        });
        app.next_id += 1;
        app.saved_recipe = app.recipe.clone();

        app.recipes = initial_recipes
            .into_iter()
            .map(|recipe| RecipeSummary {
                id: recipe.id,
                name: recipe.name,
                prep_minutes: recipe
                    .prep_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                cook_minutes: recipe
                    .cook_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                servings: recipe
                    .servings
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            })
            .collect();

        app
    }

    fn refresh_recipe_list(&mut self) {
        let pool = self.pool.clone();
        let recipes = tokio::runtime::Runtime::new().unwrap().block_on(async {
            crate::db::list_recipe_summaries(&pool)
                .await
                .unwrap_or_default()
        });

        self.recipes = recipes
            .into_iter()
            .map(|recipe| RecipeSummary {
                id: recipe.id,
                name: recipe.name,
                prep_minutes: recipe
                    .prep_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                cook_minutes: recipe
                    .cook_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                servings: recipe
                    .servings
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            })
            .collect();
    }

    fn refresh_tags(&mut self) {
        let pool = self.pool.clone();
        let tags = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::list_tags(&pool).await.unwrap_or_default() });

        self.tags = tags
            .into_iter()
            .map(|tag| TagEntry {
                edit_name: tag.name.clone(),
                tag,
            })
            .collect();
    }

    fn chapter_ids(&self) -> Vec<String> {
        self.tags
            .iter()
            .filter(|entry| entry.tag.kind == TagKind::Chapter)
            .map(|entry| entry.tag.id.clone())
            .collect()
    }

    fn add_tag(&mut self) {
        let name = self.new_tag_name.trim().to_string();
        if name.is_empty() {
            self.status = "Tag name is required.".to_string();
            return;
        }

        let pool = self.pool.clone();
        let kind = self.new_tag_kind;
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::create_tag(&pool, &name, kind).await });

        match result {
            Ok(_) => {
                self.new_tag_name.clear();
                self.status = format!("Added tag: {}", name);
            }
            Err(error) => self.status = format!("Add tag failed: {error}"),
        }
        self.refresh_tags();
    }

    fn apply_tag_action(&mut self, action: TagAction) {
        let pool = self.pool.clone();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = match action {
            TagAction::Rename(tag_id, name) => {
                if name.trim().is_empty() {
                    self.status = "Tag name is required.".to_string();
                    self.refresh_tags();
                    return;
                }
                runtime.block_on(async { crate::db::rename_tag(&pool, &tag_id, &name).await })
            }
            TagAction::Delete(tag_id) => {
                self.recipe.tag_ids.remove(&tag_id);
                self.saved_recipe.tag_ids.remove(&tag_id);
                runtime.block_on(async { crate::db::delete_tag(&pool, &tag_id).await })
            }
            TagAction::MoveChapter(index, direction) => {
                let mut chapter_ids = self.chapter_ids();
                let new_index = (index as i32 + direction) as usize;
                if new_index >= chapter_ids.len() {
                    return;
                }
                chapter_ids.swap(index, new_index);
                runtime.block_on(async { crate::db::set_tag_order(&pool, &chapter_ids).await })
            }
        };

        if let Err(error) = result {
            self.status = format!("Tag update failed: {error}");
        }
        self.refresh_tags();
    }

    fn next_entity_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn export_filename_for_date(date: chrono::NaiveDate) -> String {
        format!("{}_PlaTex.json", date.format("%Y%m%d"))
    }

    fn today_export_filename() -> String {
        Self::export_filename_for_date(Local::now().date_naive())
    }

    fn export_all_recipes_to_json(&self) -> Result<String, String> {
        let pool = self.pool.clone();
        let document = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(build_export_document(&pool))?;
        serde_json::to_string_pretty(&document)
            .map_err(|err| format!("Failed to encode JSON: {err}"))
    }

    fn export_json_file(&mut self) {
        let target = rfd::FileDialog::new()
            .set_file_name(&Self::today_export_filename())
            .add_filter("JSON", &["json"])
            .save_file();

        let Some(path) = target else {
            self.status = "Export cancelled.".to_string();
            return;
        };

        match self.export_all_recipes_to_json() {
            Ok(json) => match fs::write(&path, json) {
                Ok(_) => self.status = format!("Exported JSON to {}", path.display()),
                Err(err) => self.status = format!("Export failed: {err}"),
            },
            Err(err) => self.status = err,
        }
    }

    fn export_latex_file(&mut self) {
        let file_name = format!("{}_PlaTex.tex", Local::now().date_naive().format("%Y%m%d"));
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&file_name)
            .add_filter("LaTeX", &["tex"])
            .save_file()
        else {
            self.status = "Export cancelled.".to_string();
            return;
        };

        let pool = self.pool.clone();
        let details = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut details = Vec::new();
            for recipe in crate::db::list_recipes(&pool).await? {
                details.push(crate::db::load_recipe_detail(&pool, &recipe.id).await?);
            }
            Ok::<_, sqlx::Error>(details)
        });

        let style = match self.template_choice {
            TemplateChoice::BuiltIn(index) => crate::export::TEMPLATES[index].style,
            TemplateChoice::Uploaded => match &self.uploaded_template {
                Some(template) => template.style.as_str(),
                None => {
                    self.status = "Upload a template first.".to_string();
                    return;
                }
            },
        };
        self.status = match details {
            Ok(details) => match crate::export::write_cookbook(&path, style, &details) {
                Ok(style_path) => format!(
                    "Exported LaTeX to {} and {}",
                    path.display(),
                    style_path.display()
                ),
                Err(err) => format!("Export failed: {err}"),
            },
            Err(err) => format!("Export failed: {err}"),
        };
    }

    fn upload_template(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LaTeX style", &["sty"])
            .pick_file()
        else {
            return;
        };

        let style = match fs::read_to_string(&path) {
            Ok(style) => style,
            Err(err) => {
                self.status = format!("Could not read template: {err}");
                return;
            }
        };

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        match crate::template_check::check_template(&style) {
            Ok(()) => {
                self.status = format!("Template {} is valid.", file_name);
                self.uploaded_template = Some(UploadedTemplate { file_name, style });
                self.template_choice = TemplateChoice::Uploaded;
            }
            Err(missing) => {
                self.status = format!("Template {} is missing: {}", file_name, missing.join(", "));
            }
        }
    }

    fn delete_recipe(&mut self, recipe_id: &str, name: &str) {
        let pool = self.pool.clone();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::delete_recipe(&pool, recipe_id).await });

        match result {
            Ok(()) => {
                if self.selected_recipe_id.as_deref() == Some(recipe_id) {
                    self.reset_recipe();
                }
                self.status = format!("Deleted recipe: {name}");
            }
            Err(error) => self.status = format!("Delete recipe failed: {error}"),
        }
        self.refresh_recipe_list();
    }

    fn show_delete_confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_delete.clone() else {
            return;
        };
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new("Confirm deletion")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(pending.prompt());
                ui.horizontal(|ui| {
                    confirmed = ui.button("Delete").clicked();
                    cancelled = ui.button("Cancel").clicked();
                });
            });

        if confirmed {
            self.pending_delete = None;
            match pending {
                PendingDelete::Recipe { id, name } => self.delete_recipe(&id, &name),
                PendingDelete::Tag { id, .. } => self.apply_tag_action(TagAction::Delete(id)),
            }
        } else if cancelled {
            self.pending_delete = None;
        }
    }

    fn show_latex_export_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_latex_export;
        let mut export = false;
        egui::Window::new("Export LaTeX")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Template");
                for (index, template) in crate::export::TEMPLATES.iter().enumerate() {
                    ui.radio_value(
                        &mut self.template_choice,
                        TemplateChoice::BuiltIn(index),
                        template.name,
                    );
                }
                ui.horizontal(|ui| {
                    let label = match &self.uploaded_template {
                        Some(template) => format!("Uploaded: {}", template.file_name),
                        None => "Uploaded: none".to_string(),
                    };
                    ui.add_enabled_ui(self.uploaded_template.is_some(), |ui| {
                        ui.radio_value(&mut self.template_choice, TemplateChoice::Uploaded, label);
                    });
                    if ui.button("Upload .sty").clicked() {
                        self.upload_template();
                    }
                });
                ui.separator();
                ui.label("Chapters follow the order defined in Manage tags.");
                export = ui.button("Export").clicked();
                ui.label(format!("Status: {}", self.status));
            });

        if export {
            self.export_latex_file();
        }
        self.show_latex_export = open;
    }

    fn import_json_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            self.status = "Import cancelled.".to_string();
            return;
        };

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) => {
                self.status = format!("Import failed to read file: {err}");
                return;
            }
        };

        let document: JsonExportDocument = match serde_json::from_str(&content) {
            Ok(document) => document,
            Err(err) => {
                self.status = format!("Import failed to parse JSON: {err}");
                return;
            }
        };

        let pool = self.pool.clone();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(import_document(&pool, document, self.import_mode));

        match result {
            Ok(imported) => {
                self.refresh_recipe_list();
                self.refresh_tags();
                self.status = format!("Imported {} recipe(s) from {}", imported, path.display());
            }
            Err(err) => self.status = err,
        }
    }

    fn load_recipe_into_form(&mut self, recipe_id: String) {
        let pool = self.pool.clone();
        let detail = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::load_recipe_detail(&pool, &recipe_id).await });

        match detail {
            Ok(detail) => {
                self.selected_recipe_id = Some(recipe_id.clone());
                self.recipe.name = detail.recipe.name;
                self.recipe.description = detail.recipe.description.unwrap_or_default();
                self.recipe.servings = detail
                    .recipe
                    .servings
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                self.recipe.prep_minutes = detail
                    .recipe
                    .prep_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                self.recipe.cook_minutes = detail
                    .recipe
                    .cook_time_minutes
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                self.recipe.tag_ids = detail.tags.into_iter().map(|tag| tag.id).collect();

                self.recipe.groups.clear();
                let mut groups: Vec<IngredientGroup> = Vec::new();
                for ingredient in detail.ingredients {
                    let group_name = ingredient
                        .group_name
                        .clone()
                        .unwrap_or_else(|| "Base".to_string());
                    let quantity = ingredient.quantity.unwrap_or_default();
                    let unit = ingredient.unit.unwrap_or_default();
                    let quantity_unit = if quantity.is_empty() && unit.is_empty() {
                        String::new()
                    } else if unit.is_empty() {
                        quantity
                    } else if quantity.is_empty() {
                        unit
                    } else {
                        format!("{} {}", quantity, unit)
                    };

                    let item = IngredientItem {
                        id: self.next_entity_id(),
                        quantity_unit,
                        description: ingredient.ingredient_name,
                        optional: ingredient.optional,
                    };

                    if let Some(group) = groups.iter_mut().find(|group| group.name == group_name) {
                        group.ingredients.push(item);
                    } else {
                        groups.push(IngredientGroup {
                            id: self.next_entity_id(),
                            name: group_name,
                            ingredients: vec![item],
                        });
                    }
                }

                if groups.is_empty() {
                    let group_id = self.next_entity_id();
                    let ingredient_id = self.next_entity_id();
                    groups.push(IngredientGroup {
                        id: group_id,
                        name: "Base".to_string(),
                        ingredients: vec![IngredientItem {
                            id: ingredient_id,
                            ..Default::default()
                        }],
                    });
                }
                self.recipe.groups = groups;

                self.recipe.steps = detail
                    .steps
                    .into_iter()
                    .map(|step| PreparationStep {
                        id: self.next_entity_id(),
                        instruction: step.instruction,
                        optional: step.optional,
                    })
                    .collect();

                if self.recipe.steps.is_empty() {
                    let step_id = self.next_entity_id();
                    self.recipe.steps.push(PreparationStep {
                        id: step_id,
                        ..Default::default()
                    });
                }

                self.saved_recipe = self.recipe.clone();
                self.status = format!("Loaded: {}", self.recipe.name);
            }
            Err(error) => self.status = format!("Load failed: {error}"),
        }
    }

    /// Validates and stores the form. Returns whether the recipe was saved.
    fn save_recipe(&mut self) -> bool {
        let name = self.recipe.name.trim();
        if name.is_empty() {
            self.status = "Recipe name is required.".to_string();
            return false;
        }

        let has_valid_ingredient = self.recipe.groups.iter().any(|group| {
            group.ingredients.iter().any(|ingredient| {
                !ingredient.description.trim().is_empty()
                    || !ingredient.quantity_unit.trim().is_empty()
            })
        });

        if !has_valid_ingredient {
            self.status = "Add at least one ingredient.".to_string();
            return false;
        }

        let has_step = self
            .recipe
            .steps
            .iter()
            .any(|step| !step.instruction.trim().is_empty());

        if !has_step {
            self.status = "Add at least one preparation step.".to_string();
            return false;
        }

        let groups = self
            .recipe
            .groups
            .iter()
            .flat_map(|group| {
                group.ingredients.iter().filter_map(|ingredient| {
                    if ingredient.description.trim().is_empty()
                        && ingredient.quantity_unit.trim().is_empty()
                    {
                        None
                    } else {
                        Some(crate::db::IngredientGroupInput {
                            group_name: Some(group.name.trim().to_string()),
                            quantity_unit: ingredient.quantity_unit.trim().to_string(),
                            description: ingredient.description.trim().to_string(),
                            optional: ingredient.optional,
                        })
                    }
                })
            })
            .collect::<Vec<_>>();

        let steps = self
            .recipe
            .steps
            .iter()
            .filter(|step| !step.instruction.trim().is_empty())
            .map(|step| crate::db::StepInput {
                instruction: step.instruction.trim().to_string(),
                optional: step.optional,
            })
            .collect::<Vec<_>>();

        let saved_name = name.to_string();
        let tag_ids = self.recipe.tag_ids.iter().cloned().collect::<Vec<_>>();
        let saved_id = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let id = crate::db::upsert_recipe_with_details(
                &self.pool,
                self.selected_recipe_id.as_deref(),
                name,
                if self.recipe.description.trim().is_empty() {
                    None
                } else {
                    Some(self.recipe.description.trim())
                },
                self.recipe.servings.trim().parse::<i64>().ok(),
                self.recipe.prep_minutes.trim().parse::<i64>().ok(),
                self.recipe.cook_minutes.trim().parse::<i64>().ok(),
                &groups,
                &steps,
            )
            .await?;
            crate::db::set_recipe_tags(&self.pool, &id, &tag_ids).await?;
            Ok::<_, sqlx::Error>(id)
        });

        match saved_id {
            Ok(id) => {
                self.selected_recipe_id = Some(id);
                self.saved_recipe = self.recipe.clone();
                self.refresh_recipe_list();
                self.status = format!("Saved: {}", saved_name);
                true
            }
            Err(error) => {
                self.status = format!("Save failed: {error}");
                false
            }
        }
    }

    fn reset_recipe(&mut self) {
        self.selected_recipe_id = None;
        let group_id = self.next_entity_id();
        let ingredient_id = self.next_entity_id();
        let step_id = self.next_entity_id();
        self.recipe = RecipeDraft {
            groups: vec![IngredientGroup {
                id: group_id,
                name: "Base".to_string(),
                ingredients: vec![IngredientItem {
                    id: ingredient_id,
                    ..Default::default()
                }],
            }],
            steps: vec![PreparationStep {
                id: step_id,
                ..Default::default()
            }],
            ..Default::default()
        };
        self.saved_recipe = self.recipe.clone();
    }

    fn has_unsaved_changes(&self) -> bool {
        self.recipe != self.saved_recipe
    }

    /// Runs the action now, or holds it until the user saves or discards unsaved changes.
    fn request(&mut self, action: FormAction) {
        if self.has_unsaved_changes() {
            self.pending_action = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: FormAction) {
        match action {
            FormAction::NewRecipe => {
                self.reset_recipe();
                self.status = "New recipe".to_string();
            }
            FormAction::LoadRecipe(recipe_id) => self.load_recipe_into_form(recipe_id),
            FormAction::ExportJson => self.export_json_file(),
            FormAction::ExportLatex => self.show_latex_export = true,
            FormAction::ImportJson => self.import_json_file(),
        }
    }

    fn save_pending_action(&mut self) {
        if let Some(action) = self.pending_action.take() {
            if self.save_recipe() {
                self.perform(action);
            }
        }
    }

    fn discard_pending_action(&mut self) {
        if let Some(action) = self.pending_action.take() {
            self.recipe = self.saved_recipe.clone();
            self.perform(action);
        }
    }

    fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Recipe is not saved")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Save your changes before continuing?");
                ui.horizontal(|ui| {
                    save = ui.button("Save").clicked();
                    discard = ui.button("Discard").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });

        if save {
            self.save_pending_action();
        } else if discard {
            self.discard_pending_action();
        } else if cancel {
            self.pending_action = None;
        }
    }

    fn add_group(&mut self) {
        let group_id = self.next_entity_id();
        let ingredient_id = self.next_entity_id();
        self.recipe.groups.push(IngredientGroup {
            id: group_id,
            name: format!("Group {}", self.recipe.groups.len() + 1),
            ingredients: vec![IngredientItem {
                id: ingredient_id,
                ..Default::default()
            }],
        });
    }

    fn add_ingredient(&mut self, group_index: usize) {
        let ingredient_id = self.next_entity_id();
        if let Some(group) = self.recipe.groups.get_mut(group_index) {
            group.ingredients.push(IngredientItem {
                id: ingredient_id,
                ..Default::default()
            });
        }
    }

    fn add_step(&mut self) {
        let step_id = self.next_entity_id();
        self.recipe.steps.push(PreparationStep {
            id: step_id,
            ..Default::default()
        });
    }

    fn move_group(&mut self, index: usize, direction: i32) {
        let new_index = (index as i32 + direction) as usize;
        if new_index < self.recipe.groups.len() {
            self.recipe.groups.swap(index, new_index);
        }
    }

    fn move_ingredient(&mut self, group_index: usize, ingredient_index: usize, direction: i32) {
        let Some(group) = self.recipe.groups.get_mut(group_index) else {
            return;
        };
        let new_index = (ingredient_index as i32 + direction) as usize;
        if new_index < group.ingredients.len() {
            group.ingredients.swap(ingredient_index, new_index);
        }
    }

    fn move_step(&mut self, step_index: usize, direction: i32) {
        let new_index = (step_index as i32 + direction) as usize;
        if new_index < self.recipe.steps.len() {
            self.recipe.steps.swap(step_index, new_index);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_latex_export_window(ctx);
        self.show_delete_confirmation(ctx);
        self.show_unsaved_changes_dialog(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.heading("PlaTex");
                    ui.label("Local recipe editor");
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("New recipe").clicked() {
                            self.request(FormAction::NewRecipe);
                        }

                        if let Some(recipe_id) = self.selected_recipe_id.clone() {
                            if ui.button("Delete recipe").clicked() {
                                let name = self
                                    .recipes
                                    .iter()
                                    .find(|recipe| recipe.id == recipe_id)
                                    .map(|recipe| recipe.name.clone())
                                    .unwrap_or_else(|| self.recipe.name.clone());
                                self.pending_delete = Some(PendingDelete::Recipe {
                                    id: recipe_id,
                                    name,
                                });
                            }
                        }

                        if ui.button("Save recipe").clicked() {
                            self.save_recipe();
                        }

                        if ui.button("Export JSON").clicked() {
                            self.request(FormAction::ExportJson);
                        }

                        if ui.button("Export LaTeX...").clicked() {
                            self.request(FormAction::ExportLatex);
                        }
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut self.import_mode,
                            ImportMode::OnlyAddNew,
                            "Only add new recipes",
                        );
                        ui.radio_value(
                            &mut self.import_mode,
                            ImportMode::OverwriteChanges,
                            "Overwrite changes",
                        );
                        if ui.button("Import JSON").clicked() {
                            self.request(FormAction::ImportJson);
                        }
                    });

                    ui.separator();
                    egui::CollapsingHeader::new("Manage tags")
                        .id_salt("manage_tags")
                        .show(ui, |ui| {
                            let mut add_tag = false;
                            ui.horizontal(|ui| {
                                ui.label("New tag");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_tag_name)
                                        .desired_width(180.0),
                                );
                                ui.radio_value(&mut self.new_tag_kind, TagKind::Chapter, "Chapter");
                                ui.radio_value(&mut self.new_tag_kind, TagKind::Content, "Content");
                                add_tag = ui.button("Add tag").clicked();
                            });
                            if add_tag {
                                self.add_tag();
                            }

                            let mut actions = Vec::new();
                            let mut pending_delete = None;
                            for (kind, heading) in [
                                (TagKind::Chapter, "Chapters (in order)"),
                                (TagKind::Content, "Content tags"),
                            ] {
                                ui.label(heading);
                                let entries =
                                    self.tags.iter_mut().filter(|entry| entry.tag.kind == kind);
                                let mut count = 0;
                                for (index, entry) in entries.enumerate() {
                                    count += 1;
                                    ui.push_id(("tag", entry.tag.id.clone()), |ui| {
                                        ui.horizontal(|ui| {
                                            let response = ui.add(
                                                egui::TextEdit::singleline(&mut entry.edit_name)
                                                    .desired_width(180.0),
                                            );
                                            if response.lost_focus()
                                                && entry.edit_name != entry.tag.name
                                            {
                                                actions.push(TagAction::Rename(
                                                    entry.tag.id.clone(),
                                                    entry.edit_name.clone(),
                                                ));
                                            }
                                            if kind == TagKind::Chapter {
                                                if ui.button("up").clicked() && index > 0 {
                                                    actions.push(TagAction::MoveChapter(index, -1));
                                                }
                                                if ui.button("down").clicked() {
                                                    actions.push(TagAction::MoveChapter(index, 1));
                                                }
                                            }
                                            if ui.button("Delete").clicked() {
                                                pending_delete = Some(PendingDelete::Tag {
                                                    id: entry.tag.id.clone(),
                                                    name: entry.tag.name.clone(),
                                                });
                                            }
                                        });
                                    });
                                }
                                if count == 0 {
                                    ui.label("None yet.");
                                }
                            }
                            for action in actions {
                                self.apply_tag_action(action);
                            }
                            if pending_delete.is_some() {
                                self.pending_delete = pending_delete;
                            }
                        });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Recipe name");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.recipe.name).desired_width(220.0),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Servings");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.recipe.servings)
                                .desired_width(80.0),
                        );
                        ui.label("Prep time");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.recipe.prep_minutes)
                                .desired_width(80.0),
                        );
                        ui.label("Cook time");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.recipe.cook_minutes)
                                .desired_width(80.0),
                        );
                    });

                    ui.label("Description");
                    ui.add(egui::TextEdit::multiline(&mut self.recipe.description).desired_rows(4));

                    for (kind, label) in
                        [(TagKind::Chapter, "Chapters"), (TagKind::Content, "Tags")]
                    {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(label);
                            for entry in self.tags.iter().filter(|entry| entry.tag.kind == kind) {
                                let mut checked = self.recipe.tag_ids.contains(&entry.tag.id);
                                if ui.checkbox(&mut checked, &entry.tag.name).changed() {
                                    if checked {
                                        self.recipe.tag_ids.insert(entry.tag.id.clone());
                                    } else {
                                        self.recipe.tag_ids.remove(&entry.tag.id);
                                    }
                                }
                            }
                        });
                    }

                    ui.separator();
                    egui::CollapsingHeader::new(egui::RichText::new("Ingredients").heading())
                        .id_salt("ingredients_section")
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("ingredients_scroll")
                                .max_height(220.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut group_indexes_to_remove = Vec::new();
                                    for group_index in 0..self.recipe.groups.len() {
                                        let group_count = self.recipe.groups.len();
                                        let mut move_group_up = false;
                                        let mut move_group_down = false;
                                        let mut add_ingredient = false;
                                        let mut remove_group = false;

                                        ui.push_id(
                                            (
                                                "ingredient_group",
                                                self.recipe.groups[group_index].id,
                                            ),
                                            |ui| {
                                                ui.horizontal(|ui| {
                                                    let group =
                                                        &mut self.recipe.groups[group_index];
                                                    ui.label("Group");
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut group.name)
                                                            .desired_width(180.0),
                                                    );
                                                    move_group_up = ui.button("up").clicked()
                                                        && group_index > 0;
                                                    move_group_down = ui.button("down").clicked()
                                                        && group_index + 1 < group_count;
                                                    add_ingredient =
                                                        ui.button("Add ingredient").clicked();
                                                    remove_group =
                                                        ui.button("Remove group").clicked()
                                                            && group_count > 1;
                                                });
                                            },
                                        );

                                        if move_group_up {
                                            self.move_group(group_index, -1);
                                        }
                                        if move_group_down {
                                            self.move_group(group_index, 1);
                                        }
                                        if add_ingredient {
                                            self.add_ingredient(group_index);
                                        }
                                        if remove_group {
                                            group_indexes_to_remove.push(group_index);
                                        }

                                        let mut ingredient_indexes_to_remove = Vec::new();
                                        let mut ingredient_move_actions = Vec::new();

                                        ui.vertical(|ui| {
                                            let ingredient_count =
                                                self.recipe.groups[group_index].ingredients.len();
                                            for ingredient_index in 0..ingredient_count {
                                                let mut move_ingredient_up = false;
                                                let mut move_ingredient_down = false;
                                                let mut remove_ingredient = false;

                                                ui.push_id(
                                                    (
                                                        "ingredient",
                                                        self.recipe.groups[group_index].id,
                                                        self.recipe.groups[group_index].ingredients
                                                            [ingredient_index]
                                                            .id,
                                                    ),
                                                    |ui| {
                                                        ui.horizontal(|ui| {
                                                            let ingredient = &mut self
                                                                .recipe
                                                                .groups[group_index]
                                                                .ingredients[ingredient_index];
                                                            ui.add(
                                                                egui::TextEdit::singleline(
                                                                    &mut ingredient.quantity_unit,
                                                                )
                                                                .desired_width(150.0),
                                                            );
                                                            ui.add(
                                                                egui::TextEdit::singleline(
                                                                    &mut ingredient.description,
                                                                )
                                                                .desired_width(240.0),
                                                            );
                                                            ui.checkbox(
                                                                &mut ingredient.optional,
                                                                "Optional",
                                                            );
                                                            move_ingredient_up =
                                                                ui.button("up").clicked()
                                                                    && ingredient_index > 0;
                                                            move_ingredient_down =
                                                                ui.button("down").clicked()
                                                                    && ingredient_index + 1
                                                                        < ingredient_count;
                                                            remove_ingredient =
                                                                ui.button("Remove").clicked()
                                                                    && ingredient_count > 1;
                                                        });
                                                    },
                                                );

                                                if move_ingredient_up {
                                                    ingredient_move_actions
                                                        .push((ingredient_index, -1));
                                                }
                                                if move_ingredient_down {
                                                    ingredient_move_actions
                                                        .push((ingredient_index, 1));
                                                }
                                                if remove_ingredient {
                                                    ingredient_indexes_to_remove
                                                        .push(ingredient_index);
                                                }
                                            }
                                        });

                                        for (ingredient_index, direction) in ingredient_move_actions
                                        {
                                            self.move_ingredient(
                                                group_index,
                                                ingredient_index,
                                                direction,
                                            );
                                        }
                                        for ingredient_index in
                                            ingredient_indexes_to_remove.into_iter().rev()
                                        {
                                            if self.recipe.groups[group_index].ingredients.len() > 1
                                            {
                                                self.recipe.groups[group_index]
                                                    .ingredients
                                                    .remove(ingredient_index);
                                            }
                                        }
                                    }

                                    for group_index in group_indexes_to_remove.into_iter().rev() {
                                        if self.recipe.groups.len() > 1 {
                                            self.recipe.groups.remove(group_index);
                                        }
                                    }

                                    if ui.button("Add ingredient group").clicked() {
                                        self.add_group();
                                    }
                                });
                        });

                    ui.separator();
                    egui::CollapsingHeader::new(
                        egui::RichText::new("Preparation manual").heading(),
                    )
                    .id_salt("steps_section")
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("steps_scroll")
                            .max_height(220.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut step_indexes_to_remove = Vec::new();
                                for step_index in 0..self.recipe.steps.len() {
                                    let step_count = self.recipe.steps.len();
                                    let mut move_step_up = false;
                                    let mut move_step_down = false;
                                    let mut remove_step = false;

                                    ui.push_id(
                                        ("preparation_step", self.recipe.steps[step_index].id),
                                        |ui| {
                                            ui.horizontal(|ui| {
                                                let step = &mut self.recipe.steps[step_index];
                                                ui.label(format!("Step {}", step_index + 1));
                                                ui.checkbox(&mut step.optional, "Optional");
                                                move_step_up =
                                                    ui.button("up").clicked() && step_index > 0;
                                                move_step_down = ui.button("down").clicked()
                                                    && step_index + 1 < step_count;
                                                remove_step =
                                                    ui.button("Remove").clicked() && step_count > 1;
                                            });

                                            ui.add(
                                                egui::TextEdit::multiline(
                                                    &mut self.recipe.steps[step_index].instruction,
                                                )
                                                .desired_rows(3),
                                            );
                                        },
                                    );

                                    if move_step_up {
                                        self.move_step(step_index, -1);
                                    }
                                    if move_step_down {
                                        self.move_step(step_index, 1);
                                    }
                                    if remove_step {
                                        step_indexes_to_remove.push(step_index);
                                    }
                                }

                                for step_index in step_indexes_to_remove.into_iter().rev() {
                                    if self.recipe.steps.len() > 1 {
                                        self.recipe.steps.remove(step_index);
                                    }
                                }

                                if ui.button("Add preparation step").clicked() {
                                    self.add_step();
                                }
                            });
                    });

                    ui.separator();
                    ui.label(format!("Status: {}", self.status));

                    ui.separator();
                    ui.heading("Recipes");
                    egui::ScrollArea::vertical()
                        .id_salt("recipes_scroll")
                        .max_height(180.0)
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            if self.recipes.is_empty() {
                                ui.label("No recipes yet.");
                            } else {
                                for (recipe_index, recipe) in
                                    self.recipes.clone().into_iter().enumerate()
                                {
                                    let recipe_id = recipe.id.clone();
                                    ui.push_id(
                                        ("saved_recipe", recipe_index, recipe_id.clone()),
                                        |ui| {
                                            ui.horizontal(|ui| {
                                                if ui.button(&recipe.name).clicked() {
                                                    self.request(FormAction::LoadRecipe(
                                                        recipe_id.clone(),
                                                    ));
                                                }
                                                if !recipe.prep_minutes.is_empty() {
                                                    ui.label(format!(
                                                        "Prep {} min",
                                                        recipe.prep_minutes
                                                    ));
                                                }
                                                if !recipe.cook_minutes.is_empty() {
                                                    ui.label(format!(
                                                        "Cook {} min",
                                                        recipe.cook_minutes
                                                    ));
                                                }
                                                if !recipe.servings.is_empty() {
                                                    ui.label(format!("Serves {}", recipe.servings));
                                                }
                                            });
                                        },
                                    );
                                }
                            }
                        });
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::App;

    #[test]
    fn export_filename_uses_yyyymmdd_prefix() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        assert_eq!(App::export_filename_for_date(date), "20261003_PlaTex.json");
    }

    #[test]
    fn json_document_round_trips() {
        let document = super::JsonExportDocument {
            version: 2,
            exported_at: "2026-10-03T00:00:00Z".to_string(),
            tags: vec![super::JsonTag {
                uuid: "t1".to_string(),
                name: "Soups".to_string(),
                kind: super::TagKind::Chapter,
                sort_order: 0,
            }],
            recipes: vec![super::JsonRecipe {
                uuid: Some("b0f0e9ab-5c38-4dc3-9c3f-f9ddc89c0338".to_string()),
                name: "Tomato Soup".to_string(),
                description: Some("Warm and simple".to_string()),
                servings: Some(2),
                prep_time_minutes: Some(10),
                cook_time_minutes: Some(20),
                ingredients: vec![super::JsonIngredient {
                    group_name: Some("Base".to_string()),
                    quantity_unit: "2 cups".to_string(),
                    description: "tomato".to_string(),
                    optional: false,
                }],
                steps: vec![super::JsonStep {
                    instruction: "Boil and simmer.".to_string(),
                    optional: false,
                }],
                tags: vec!["t1".to_string()],
            }],
        };

        let json = serde_json::to_string_pretty(&document).unwrap();
        let parsed: super::JsonExportDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, document);
    }

    #[test]
    fn version_1_document_without_tags_parses() {
        let json = r#"{"version":1,"exported_at":"x","recipes":[{"name":"Soup","description":null,"servings":null,"prep_time_minutes":null,"cook_time_minutes":null,"ingredients":[],"steps":[]}]}"#;
        let parsed: super::JsonExportDocument = serde_json::from_str(json).unwrap();
        assert!(parsed.tags.is_empty());
        assert!(parsed.recipes[0].tags.is_empty());
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = crate::db::open_test_db().await;
        pool
    }

    fn tagged_document(tag_name: &str, recipe_name: &str) -> super::JsonExportDocument {
        super::JsonExportDocument {
            version: 2,
            exported_at: "2026-10-03T00:00:00Z".to_string(),
            tags: vec![super::JsonTag {
                uuid: "t1".to_string(),
                name: tag_name.to_string(),
                kind: super::TagKind::Chapter,
                sort_order: 0,
            }],
            recipes: vec![super::JsonRecipe {
                uuid: Some("r1".to_string()),
                name: recipe_name.to_string(),
                description: None,
                servings: None,
                prep_time_minutes: None,
                cook_time_minutes: None,
                ingredients: vec![],
                steps: vec![],
                tags: vec!["t1".to_string(), "unknown".to_string()],
            }],
        }
    }

    #[tokio::test]
    async fn export_then_import_keeps_tags_and_uuids() {
        let source = test_pool().await;
        let tag_id = crate::db::create_tag(&source, "Soups", super::TagKind::Chapter)
            .await
            .unwrap();
        let recipe_id = crate::db::insert_recipe(&source, "Soup", None, None, None, None)
            .await
            .unwrap();
        crate::db::set_recipe_tags(&source, &recipe_id, &[tag_id.clone()])
            .await
            .unwrap();

        let document = super::build_export_document(&source).await.unwrap();
        let target = test_pool().await;
        super::import_document(&target, document, super::ImportMode::OverwriteChanges)
            .await
            .unwrap();

        let tags = crate::db::list_tags_for_recipe(&target, &recipe_id)
            .await
            .unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].id, tag_id);
        assert_eq!(tags[0].name, "Soups");
        assert_eq!(tags[0].kind, super::TagKind::Chapter);
    }

    #[tokio::test]
    async fn only_add_new_keeps_existing_tags_and_recipes() {
        let pool = test_pool().await;
        super::import_document(
            &pool,
            tagged_document("Soups", "Soup"),
            super::ImportMode::OnlyAddNew,
        )
        .await
        .unwrap();

        let imported = super::import_document(
            &pool,
            tagged_document("Stews", "Stew"),
            super::ImportMode::OnlyAddNew,
        )
        .await
        .unwrap();

        assert_eq!(imported, 0);
        assert_eq!(crate::db::list_tags(&pool).await.unwrap()[0].name, "Soups");
        assert_eq!(
            crate::db::list_recipes(&pool).await.unwrap()[0].name,
            "Soup"
        );
    }

    #[tokio::test]
    async fn overwrite_changes_updates_tags_and_skips_unknown_tag_refs() {
        let pool = test_pool().await;
        super::import_document(
            &pool,
            tagged_document("Soups", "Soup"),
            super::ImportMode::OverwriteChanges,
        )
        .await
        .unwrap();
        super::import_document(
            &pool,
            tagged_document("Stews", "Stew"),
            super::ImportMode::OverwriteChanges,
        )
        .await
        .unwrap();

        let tags = crate::db::list_tags_for_recipe(&pool, "r1").await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Stews");
        assert_eq!(crate::db::list_tags(&pool).await.unwrap().len(), 1);
        assert_eq!(
            crate::db::list_recipes(&pool).await.unwrap()[0].name,
            "Stew"
        );
    }

    /// `App` runs its own runtimes, so it must be built outside of one.
    fn test_app() -> (tokio::runtime::Runtime, App) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let pool = runtime.block_on(crate::db::open_test_db());
        (runtime, App::new(std::sync::Arc::new(pool), Vec::new()))
    }

    #[test]
    fn actions_run_immediately_without_unsaved_changes() {
        let (_runtime, mut app) = test_app();
        app.request(super::FormAction::ExportLatex);

        assert!(app.pending_action.is_none());
        assert!(app.show_latex_export);
    }

    #[test]
    fn unsaved_changes_hold_action_until_discarded() {
        let (_runtime, mut app) = test_app();
        app.recipe.name = "Draft".to_string();
        app.request(super::FormAction::ExportLatex);

        assert_eq!(app.pending_action, Some(super::FormAction::ExportLatex));
        assert!(!app.show_latex_export);

        app.discard_pending_action();

        assert!(app.pending_action.is_none());
        assert!(app.show_latex_export);
        assert_eq!(app.recipe.name, "");
        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn saving_pending_action_stores_recipe_then_runs_action() {
        let (_runtime, mut app) = test_app();
        app.recipe.name = "Soup".to_string();
        app.recipe.groups[0].ingredients[0].description = "water".to_string();
        app.recipe.steps[0].instruction = "Boil.".to_string();
        app.request(super::FormAction::NewRecipe);

        app.save_pending_action();

        assert!(app.pending_action.is_none());
        assert_eq!(app.recipe.name, "");
        assert!(app.selected_recipe_id.is_none());
        assert_eq!(app.recipes.len(), 1);
        assert_eq!(app.recipes[0].name, "Soup");
    }

    #[test]
    fn failed_save_keeps_changes_and_skips_action() {
        let (_runtime, mut app) = test_app();
        app.recipe.name = "No ingredients".to_string();
        app.request(super::FormAction::NewRecipe);

        app.save_pending_action();

        assert!(app.pending_action.is_none());
        assert_eq!(app.recipe.name, "No ingredients");
        assert!(app.has_unsaved_changes());
    }
}
