use eframe::egui;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IngredientItem {
    quantity_unit: String,
    description: String,
    optional: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IngredientGroup {
    name: String,
    ingredients: Vec<IngredientItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PreparationStep {
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
    name: String,
    prep_minutes: String,
    cook_minutes: String,
    servings: String,
}

#[derive(Default)]
pub struct App {
    recipe: RecipeDraft,
    recipes: Vec<RecipeSummary>,
    status: String,
}

impl App {
    pub fn new(recipe_count: usize) -> Self {
        let mut app = Self::default();
        app.recipe.groups.push(IngredientGroup {
            name: "Base".to_string(),
            ingredients: vec![IngredientItem::default()],
        });
        app.recipe.steps.push(PreparationStep::default());

        for index in 0..recipe_count.min(5) {
            app.recipes.push(RecipeSummary {
                name: format!("Recipe {index}"),
                prep_minutes: (15 + index * 5).to_string(),
                cook_minutes: (10 + index * 7).to_string(),
                servings: (2 + (index % 4)).to_string(),
            });
        }

        app.status = "Ready".to_string();
        app
    }

    fn reset_recipe(&mut self) {
        self.recipe = RecipeDraft {
            groups: vec![IngredientGroup {
                name: "Base".to_string(),
                ingredients: vec![IngredientItem::default()],
            }],
            steps: vec![PreparationStep::default()],
            ..Default::default()
        };
    }

    fn add_group(&mut self) {
        self.recipe.groups.push(IngredientGroup {
            name: format!("Group {}", self.recipe.groups.len() + 1),
            ingredients: vec![IngredientItem::default()],
        });
    }

    fn add_ingredient(&mut self, group_index: usize) {
        if let Some(group) = self.recipe.groups.get_mut(group_index) {
            group.ingredients.push(IngredientItem::default());
        }
    }

    fn add_step(&mut self) {
        self.recipe.steps.push(PreparationStep::default());
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

                    self.recipes.insert(
                        0,
                        RecipeSummary {
                            name: name.to_string(),
                            prep_minutes: self.recipe.prep_minutes.trim().to_string(),
                            cook_minutes: self.recipe.cook_minutes.trim().to_string(),
                            servings: self.recipe.servings.trim().to_string(),
                        },
                    );

                    self.status = format!("Saved: {}", name);
                    self.reset_recipe();
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

            let mut group_indexes_to_remove = Vec::new();
            for group_index in 0..self.recipe.groups.len() {
                let group_count = self.recipe.groups.len();
                let mut move_group_up = false;
                let mut move_group_down = false;
                let mut add_ingredient = false;
                let mut remove_group = false;

                ui.horizontal(|ui| {
                    let group = &mut self.recipe.groups[group_index];
                    ui.label("Group");
                    ui.add(egui::TextEdit::singleline(&mut group.name).desired_width(180.0));
                    move_group_up = ui.button("up").clicked() && group_index > 0;
                    move_group_down = ui.button("down").clicked() && group_index + 1 < group_count;
                    add_ingredient = ui.button("Add ingredient").clicked();
                    remove_group = ui.button("Remove group").clicked() && group_count > 1;
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
                            move_ingredient_up =
                                ui.small_button("up").clicked() && ingredient_index > 0;
                            move_ingredient_down = ui.small_button("down").clicked()
                                && ingredient_index + 1 < ingredient_count;
                            remove_ingredient =
                                ui.button("Remove").clicked() && ingredient_count > 1;
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

            ui.separator();
            ui.heading("Preparation manual");

            let mut step_indexes_to_remove = Vec::new();
            for step_index in 0..self.recipe.steps.len() {
                let step_count = self.recipe.steps.len();
                let mut move_step_up = false;
                let mut move_step_down = false;
                let mut remove_step = false;

                ui.horizontal(|ui| {
                    let step = &mut self.recipe.steps[step_index];
                    ui.label(format!("Step {}", step_index + 1));
                    ui.checkbox(&mut step.optional, "Optional");
                    move_step_up = ui.button("up").clicked() && step_index > 0;
                    move_step_down = ui.button("down").clicked() && step_index + 1 < step_count;
                    remove_step = ui.button("Remove").clicked() && step_count > 1;
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

                ui.add(
                    egui::TextEdit::multiline(&mut self.recipe.steps[step_index].instruction)
                        .desired_rows(3),
                );
            }

            for step_index in step_indexes_to_remove.into_iter().rev() {
                if self.recipe.steps.len() > 1 {
                    self.recipe.steps.remove(step_index);
                }
            }

            if ui.button("Add preparation step").clicked() {
                self.add_step();
            }

            ui.separator();
            ui.label(format!("Status: {}", self.status));

            ui.separator();
            ui.heading("Recipes");
            if self.recipes.is_empty() {
                ui.label("No recipes yet.");
            } else {
                for recipe in &self.recipes {
                    ui.horizontal(|ui| {
                        ui.label(&recipe.name);
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
                }
            }
        });
    }
}
