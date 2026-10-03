#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub servings: Option<i64>,
    pub prep_time_minutes: Option<i64>,
    pub cook_time_minutes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TagKind {
    /// Groups recipes into ordered chapters.
    Chapter,
    /// Describes recipe content, e.g. breakfast or vegan.
    #[default]
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub kind: TagKind,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Ingredient {
    pub id: i64,
    pub recipe_id: String,
    pub group_name: Option<String>,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub ingredient_name: String,
    pub optional: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Step {
    pub id: i64,
    pub recipe_id: String,
    pub step_number: i64,
    pub instruction: String,
    pub optional: bool,
    pub sort_order: i64,
}

/// A recipe with all related data, as loaded for editing or export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeDetail {
    pub recipe: Recipe,
    pub ingredients: Vec<Ingredient>,
    pub steps: Vec<Step>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct RecipeTag {
    pub recipe_id: String,
    pub tag_id: String,
}
