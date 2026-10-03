use std::sync::Arc;

use eframe::egui;

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecipeSummary {
    id: i64,
    name: String,
    prep_minutes: String,
    cook_minutes: String,
    servings: String,
}

pub struct App {
    pool: Arc<sqlx::SqlitePool>,
    selected_recipe_id: Option<i64>,
    recipe: RecipeDraft,
    recipes: Vec<RecipeSummary>,
    status: String,
    next_id: u64,
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
        };

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

        app.recipes = initial_recipes
            .into_iter()
            .map(|recipe| RecipeSummary {
                id: recipe.id,
                name: recipe.name,
                prep_minutes: recipe.prep_time_minutes.map(|value| value.to_string()).unwrap_or_default(),
                cook_minutes: recipe.cook_time_minutes.map(|value| value.to_string()).unwrap_or_default(),
                servings: recipe.servings.map(|value| value.to_string()).unwrap_or_default(),
            })
            .collect();

        app
    }

    fn refresh_recipe_list(&mut self) {
        let pool = self.pool.clone();
        let recipes = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::list_recipe_summaries(&pool).await.unwrap_or_default() });

        self.recipes = recipes
            .into_iter()
            .map(|recipe| RecipeSummary {
                id: recipe.id,
                name: recipe.name,
                prep_minutes: recipe.prep_time_minutes.map(|value| value.to_string()).unwrap_or_default(),
                cook_minutes: recipe.cook_time_minutes.map(|value| value.to_string()).unwrap_or_default(),
                servings: recipe.servings.map(|value| value.to_string()).unwrap_or_default(),
            })
            .collect();
    }

    fn next_entity_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn load_recipe_into_form(&mut self, recipe_id: i64) {
        let pool = self.pool.clone();
        let detail = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { crate::db::load_recipe_detail(&pool, recipe_id).await });

        match detail {
            Ok(detail) => {
                self.selected_recipe_id = Some(recipe_id);
                self.recipe.name = detail.recipe.name;
                self.recipe.description = detail.recipe.description.unwrap_or_default();
                self.recipe.servings = detail.recipe.servings.map(|value| value.to_string()).unwrap_or_default();
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

                self.recipe.groups.clear();
                let mut groups: Vec<IngredientGroup> = Vec::new();
                for ingredient in detail.ingredients {
                    let group_name = ingredient.group_name.clone().unwrap_or_else(|| "Base".to_string());
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

                self.status = format!("Loaded: {}", self.recipe.name);
            }
            Err(error) => self.status = format!("Load failed: {error}"),
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PlaTex");
            ui.label("Local recipe editor");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("New recipe").clicked() {
                    self.reset_recipe();
                    self.status = "New recipe".to_string();
                }

                if ui.button("Save recipe").clicked() {
                    let name = self.recipe.name.trim();
                    if name.is_empty() {
                        self.status = "Recipe name is required.".to_string();
                        return;
                    }

                    let has_valid_ingredient = self.recipe.groups.iter().any(|group| {
                        group.ingredients.iter().any(|ingredient| {
                            !ingredient.description.trim().is_empty()
                                || !ingredient.quantity_unit.trim().is_empty()
                        })
                    });

                    if !has_valid_ingredient {
                        self.status = "Add at least one ingredient.".to_string();
                        return;
                    }

                    let has_step = self
                        .recipe
                        .steps
                        .iter()
                        .any(|step| !step.instruction.trim().is_empty());

                    if !has_step {
                        self.status = "Add at least one preparation step.".to_string();
                        return;
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
                    let saved_id = tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(async {
                            crate::db::upsert_recipe_with_details(
                                &self.pool,
                                self.selected_recipe_id,
                                name,
                                if self.recipe.description.trim().is_empty() {
                                    None
                                } else {
                                    Some(self.recipe.description.trim())
                                },
                                self.recipe
                                    .servings
                                    .trim()
                                    .parse::<i64>()
                                    .ok(),
                                self.recipe
                                    .prep_minutes
                                    .trim()
                                    .parse::<i64>()
                                    .ok(),
                                self.recipe
                                    .cook_minutes
                                    .trim()
                                    .parse::<i64>()
                                    .ok(),
                                &groups,
                                &steps,
                            )
                            .await
                        });

                    match saved_id {
                        Ok(id) => {
                            self.selected_recipe_id = Some(id);
                            self.refresh_recipe_list();
                            self.status = format!("Saved: {}", saved_name);
                        }
                        Err(error) => self.status = format!("Save failed: {error}"),
                    }
                }

                if ui.button("Export LaTeX").clicked() {
                    self.status = "Export ready".to_string();
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Recipe name");
                ui.add(egui::TextEdit::singleline(&mut self.recipe.name).desired_width(220.0));
            });

            ui.horizontal(|ui| {
                ui.label("Servings");
                ui.add(egui::TextEdit::singleline(&mut self.recipe.servings).desired_width(80.0));
                ui.label("Prep time");
                ui.add(
                    egui::TextEdit::singleline(&mut self.recipe.prep_minutes).desired_width(80.0),
                );
                ui.label("Cook time");
                ui.add(
                    egui::TextEdit::singleline(&mut self.recipe.cook_minutes).desired_width(80.0),
                );
            });

            ui.label("Description");
            ui.add(egui::TextEdit::multiline(&mut self.recipe.description).desired_rows(4));

            ui.separator();
            ui.heading("Ingredients");
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

                        ui.push_id(("ingredient_group", self.recipe.groups[group_index].id), |ui| {
                            ui.horizontal(|ui| {
                                let group = &mut self.recipe.groups[group_index];
                                ui.label("Group");
                                ui.add(egui::TextEdit::singleline(&mut group.name).desired_width(180.0));
                                move_group_up = ui.button("up").clicked() && group_index > 0;
                                move_group_down = ui.button("down").clicked() && group_index + 1 < group_count;
                                add_ingredient = ui.button("Add ingredient").clicked();
                                remove_group = ui.button("Remove group").clicked() && group_count > 1;
                            });
                        });

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
                            let ingredient_count = self.recipe.groups[group_index].ingredients.len();
                            for ingredient_index in 0..ingredient_count {
                                let mut move_ingredient_up = false;
                                let mut move_ingredient_down = false;
                                let mut remove_ingredient = false;

                                ui.push_id(("ingredient", self.recipe.groups[group_index].id, self.recipe.groups[group_index].ingredients[ingredient_index].id), |ui| {
                                    ui.horizontal(|ui| {
                                        let ingredient =
                                            &mut self.recipe.groups[group_index].ingredients[ingredient_index];
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ingredient.quantity_unit)
                                                .desired_width(150.0),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ingredient.description)
                                                .desired_width(240.0),
                                        );
                                        ui.checkbox(&mut ingredient.optional, "Optional");
                                        move_ingredient_up = ui.button("up").clicked() && ingredient_index > 0;
                                        move_ingredient_down = ui.button("down").clicked()
                                            && ingredient_index + 1 < ingredient_count;
                                        remove_ingredient =
                                            ui.button("Remove").clicked() && ingredient_count > 1;
                                    });
                                });

                                if move_ingredient_up {
                                    ingredient_move_actions.push((ingredient_index, -1));
                                }
                                if move_ingredient_down {
                                    ingredient_move_actions.push((ingredient_index, 1));
                                }
                                if remove_ingredient {
                                    ingredient_indexes_to_remove.push(ingredient_index);
                                }
                            }
                        });

                        for (ingredient_index, direction) in ingredient_move_actions {
                            self.move_ingredient(group_index, ingredient_index, direction);
                        }
                        for ingredient_index in ingredient_indexes_to_remove.into_iter().rev() {
                            if self.recipe.groups[group_index].ingredients.len() > 1 {
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

            ui.separator();
            ui.heading("Preparation manual");
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

                        ui.push_id(("preparation_step", self.recipe.steps[step_index].id), |ui| {
                            ui.horizontal(|ui| {
                                let step = &mut self.recipe.steps[step_index];
                                ui.label(format!("Step {}", step_index + 1));
                                ui.checkbox(&mut step.optional, "Optional");
                                move_step_up = ui.button("up").clicked() && step_index > 0;
                                move_step_down = ui.button("down").clicked() && step_index + 1 < step_count;
                                remove_step = ui.button("Remove").clicked() && step_count > 1;
                            });

                            ui.add(
                                egui::TextEdit::multiline(&mut self.recipe.steps[step_index].instruction)
                                    .desired_rows(3),
                            );
                        });

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

            ui.separator();
            ui.label(format!("Status: {}", self.status));

            ui.separator();
            ui.heading("Recipes");
            egui::ScrollArea::vertical()
                .id_salt("recipes_scroll")
                .max_height(180.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.recipes.is_empty() {
                        ui.label("No recipes yet.");
                    } else {
                        for (recipe_index, recipe) in self.recipes.clone().into_iter().enumerate() {
                            ui.push_id(("saved_recipe", recipe_index, recipe.id), |ui| {
                                ui.horizontal(|ui| {
                                    if ui.button(&recipe.name).clicked() {
                                        self.load_recipe_into_form(recipe.id);
                                    }
                                    if !recipe.prep_minutes.is_empty() {
                                        ui.label(format!("Prep {} min", recipe.prep_minutes));
                                    }
                                    if !recipe.cook_minutes.is_empty() {
                                        ui.label(format!("Cook {} min", recipe.cook_minutes));
                                    }
                                    if !recipe.servings.is_empty() {
                                        ui.label(format!("Serves {}", recipe.servings));
                                    }
                                });
                            });
                        }
                    }
                });
        });
    }
}
