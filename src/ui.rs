use eframe::egui;

#[derive(Default)]
pub struct App {
    recipe_name: String,
    description: String,
    servings: String,
    prep_minutes: String,
    cook_minutes: String,
    recipes: Vec<String>,
    status: String,
}

impl App {
    pub fn new(recipe_count: usize) -> Self {
        let mut app = Self::default();
        app.recipes = (0..recipe_count.min(5)).map(|n| format!("Recipe {n}")).collect();
        app.status = "Ready".to_string();
        app
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PlaTex");
            ui.label("Local recipe editor");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Recipe name");
                ui.text_edit_singleline(&mut self.recipe_name);
            });

            ui.horizontal(|ui| {
                ui.label("Servings");
                ui.add(egui::TextEdit::singleline(&mut self.servings).desired_width(80.0));
            });

            ui.horizontal(|ui| {
                ui.label("Prep");
                ui.add(egui::TextEdit::singleline(&mut self.prep_minutes).desired_width(80.0));
                ui.label("Cook");
                ui.add(egui::TextEdit::singleline(&mut self.cook_minutes).desired_width(80.0));
            });

            ui.label("Description");
            ui.add(egui::TextEdit::multiline(&mut self.description).desired_rows(4));

            ui.horizontal(|ui| {
                if ui.button("Save recipe").clicked() {
                    self.status = format!("Saved: {}", self.recipe_name.trim());
                    if !self.recipe_name.trim().is_empty() {
                        self.recipes.insert(0, self.recipe_name.clone());
                        self.recipe_name.clear();
                        self.description.clear();
                        self.servings.clear();
                        self.prep_minutes.clear();
                        self.cook_minutes.clear();
                    }
                }

                if ui.button("Export LaTeX").clicked() {
                    self.status = "Export ready".to_string();
                }
            });

            ui.separator();
            ui.label(format!("Status: {}", self.status));

            ui.separator();
            ui.heading("Recipes");
            if self.recipes.is_empty() {
                ui.label("No recipes yet.");
            } else {
                for recipe in &self.recipes {
                    ui.label(recipe);
                }
            }
        });
    }
}
