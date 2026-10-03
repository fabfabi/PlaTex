use sqlx::SqlitePool;

use crate::models::{Ingredient, Recipe, Step, Tag};

pub async fn connect_db(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePool::connect(database_url).await
}

pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recipes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            servings INTEGER,
            prep_time_minutes INTEGER,
            cook_time_minutes INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recipe_tags (
            recipe_id TEXT NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (recipe_id, tag_id),
            FOREIGN KEY (recipe_id) REFERENCES recipes(id),
            FOREIGN KEY (tag_id) REFERENCES tags(id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ingredients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recipe_id TEXT NOT NULL,
            group_name TEXT,
            quantity TEXT,
            unit TEXT,
            ingredient_name TEXT NOT NULL,
            optional BOOLEAN NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (recipe_id) REFERENCES recipes(id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS steps (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recipe_id TEXT NOT NULL,
            step_number INTEGER NOT NULL,
            instruction TEXT NOT NULL,
            optional BOOLEAN NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (recipe_id) REFERENCES recipes(id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_recipes(pool: &SqlitePool) -> Result<Vec<Recipe>, sqlx::Error> {
    let recipes = sqlx::query_as::<_, Recipe>(
        r#"
        SELECT id, name, description, servings, prep_time_minutes, cook_time_minutes, created_at, updated_at
        FROM recipes
        ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(recipes)
}

pub async fn clear_all_recipes(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM recipe_tags").execute(pool).await?;
    sqlx::query("DELETE FROM ingredients").execute(pool).await?;
    sqlx::query("DELETE FROM steps").execute(pool).await?;
    sqlx::query("DELETE FROM tags").execute(pool).await?;
    sqlx::query("DELETE FROM recipes").execute(pool).await?;
    Ok(())
}

pub async fn list_ingredients_for_recipe(
    pool: &SqlitePool,
    recipe_id: &str,
) -> Result<Vec<Ingredient>, sqlx::Error> {
    let ingredients = sqlx::query_as::<_, Ingredient>(
        r#"
        SELECT id, recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order
        FROM ingredients
        WHERE recipe_id = ?
        ORDER BY sort_order
        "#,
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await?;

    Ok(ingredients)
}

pub async fn list_steps_for_recipe(
    pool: &SqlitePool,
    recipe_id: &str,
) -> Result<Vec<Step>, sqlx::Error> {
    let steps = sqlx::query_as::<_, Step>(
        r#"
        SELECT id, recipe_id, step_number, instruction, optional, sort_order
        FROM steps
        WHERE recipe_id = ?
        ORDER BY sort_order
        "#,
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await?;

    Ok(steps)
}

pub async fn list_tags_for_recipe(
    pool: &SqlitePool,
    recipe_id: &str,
) -> Result<Vec<Tag>, sqlx::Error> {
    let tags = sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.id, t.name
        FROM tags t
        INNER JOIN recipe_tags rt ON rt.tag_id = t.id
        WHERE rt.recipe_id = ?
        ORDER BY t.name
        "#,
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await?;

    Ok(tags)
}

pub async fn insert_recipe(
    pool: &SqlitePool,
    name: &str,
    description: Option<&str>,
    servings: Option<i64>,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
) -> Result<String, sqlx::Error> {
    let recipe_id = uuid::Uuid::new_v4().to_string();
    insert_recipe_with_id(pool, &recipe_id, name, description, servings, prep_time_minutes, cook_time_minutes).await?;
    Ok(recipe_id)
}

pub async fn insert_recipe_with_id(
    pool: &SqlitePool,
    recipe_id: &str,
    name: &str,
    description: Option<&str>,
    servings: Option<i64>,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO recipes (id, name, description, servings, prep_time_minutes, cook_time_minutes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
        "#,
    )
    .bind(recipe_id)
    .bind(name)
    .bind(description)
    .bind(servings)
    .bind(prep_time_minutes)
    .bind(cook_time_minutes)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientGroupInput {
    pub group_name: Option<String>,
    pub quantity_unit: String,
    pub description: String,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepInput {
    pub instruction: String,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeDetail {
    pub recipe: Recipe,
    pub ingredients: Vec<Ingredient>,
    pub steps: Vec<Step>,
}

fn split_quantity_and_unit(value: &str) -> (Option<String>, Option<String>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return (None, None);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return (None, None);
    }

    let quantity = Some(parts[0].to_string());
    let unit = if parts.len() > 1 {
        Some(parts[1..].join(" "))
    } else {
        None
    };

    (quantity, unit)
}

pub async fn save_recipe_with_details(
    pool: &SqlitePool,
    name: &str,
    description: Option<&str>,
    servings: Option<i64>,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
    groups: &[IngredientGroupInput],
    steps: &[StepInput],
) -> Result<String, sqlx::Error> {
    let recipe_id = insert_recipe(
        pool,
        name,
        description,
        servings,
        prep_time_minutes,
        cook_time_minutes,
    )
    .await?;

    for (index, group) in groups.iter().enumerate() {
        let trimmed_description = group.description.trim();
        let trimmed_quantity = group.quantity_unit.trim();
        if trimmed_description.is_empty() && trimmed_quantity.is_empty() {
            continue;
        }

        let (quantity, unit) = split_quantity_and_unit(trimmed_quantity);

        sqlx::query(
            r#"
            INSERT INTO ingredients (recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&recipe_id)
        .bind(group.group_name.as_deref().filter(|value| !value.trim().is_empty()))
        .bind(quantity)
        .bind(unit)
        .bind(trimmed_description)
        .bind(group.optional)
        .bind(index as i64)
        .execute(pool)
        .await?;
    }

    for (index, step) in steps.iter().enumerate() {
        let trimmed_instruction = step.instruction.trim();
        if trimmed_instruction.is_empty() {
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO steps (recipe_id, step_number, instruction, optional, sort_order)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&recipe_id)
        .bind(index as i64 + 1)
        .bind(trimmed_instruction)
        .bind(step.optional)
        .bind(index as i64)
        .execute(pool)
        .await?;
    }

    Ok(recipe_id)
}

pub async fn upsert_recipe_with_details(
    pool: &SqlitePool,
    recipe_id: Option<&str>,
    name: &str,
    description: Option<&str>,
    servings: Option<i64>,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
    groups: &[IngredientGroupInput],
    steps: &[StepInput],
) -> Result<String, sqlx::Error> {
    if let Some(id) = recipe_id {
        let existing_recipe = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM recipes
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        if existing_recipe > 0 {
            sqlx::query(
                r#"
                UPDATE recipes
                SET name = ?, description = ?, servings = ?, prep_time_minutes = ?, cook_time_minutes = ?, updated_at = datetime('now')
                WHERE id = ?
                "#,
            )
            .bind(name)
            .bind(description)
            .bind(servings)
            .bind(prep_time_minutes)
            .bind(cook_time_minutes)
            .bind(id)
            .execute(pool)
            .await?;

            sqlx::query("DELETE FROM ingredients WHERE recipe_id = ?")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM steps WHERE recipe_id = ?")
                .bind(id)
                .execute(pool)
                .await?;

            let mut next_group_sort = 0i64;
            for group in groups {
                let trimmed_description = group.description.trim();
                let trimmed_quantity = group.quantity_unit.trim();
                if trimmed_description.is_empty() && trimmed_quantity.is_empty() {
                    continue;
                }

                let (quantity, unit) = split_quantity_and_unit(trimmed_quantity);
                sqlx::query(
                    r#"
                    INSERT INTO ingredients (recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(id)
                .bind(group.group_name.as_deref().filter(|value| !value.trim().is_empty()))
                .bind(quantity)
                .bind(unit)
                .bind(trimmed_description)
                .bind(group.optional)
                .bind(next_group_sort)
                .execute(pool)
                .await?;
                next_group_sort += 1;
            }

            let mut next_step_sort = 0i64;
            for step in steps {
                let trimmed_instruction = step.instruction.trim();
                if trimmed_instruction.is_empty() {
                    continue;
                }

                sqlx::query(
                    r#"
                    INSERT INTO steps (recipe_id, step_number, instruction, optional, sort_order)
                    VALUES (?, ?, ?, ?, ?)
                    "#,
                )
                .bind(id)
                .bind(next_step_sort + 1)
                .bind(trimmed_instruction)
                .bind(step.optional)
                .bind(next_step_sort)
                .execute(pool)
                .await?;
                next_step_sort += 1;
            }

            return Ok(id.to_string());
        }
    }

    if let Some(id) = recipe_id {
        insert_recipe_with_id(
            pool,
            id,
            name,
            description,
            servings,
            prep_time_minutes,
            cook_time_minutes,
        )
        .await?;
        let mut next_group_sort = 0i64;
        for group in groups {
            let trimmed_description = group.description.trim();
            let trimmed_quantity = group.quantity_unit.trim();
            if trimmed_description.is_empty() && trimmed_quantity.is_empty() {
                continue;
            }

            let (quantity, unit) = split_quantity_and_unit(trimmed_quantity);
            sqlx::query(
                r#"
                INSERT INTO ingredients (recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(group.group_name.as_deref().filter(|value| !value.trim().is_empty()))
            .bind(quantity)
            .bind(unit)
            .bind(trimmed_description)
            .bind(group.optional)
            .bind(next_group_sort)
            .execute(pool)
            .await?;
            next_group_sort += 1;
        }

        let mut next_step_sort = 0i64;
        for step in steps {
            let trimmed_instruction = step.instruction.trim();
            if trimmed_instruction.is_empty() {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO steps (recipe_id, step_number, instruction, optional, sort_order)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(next_step_sort + 1)
            .bind(trimmed_instruction)
            .bind(step.optional)
            .bind(next_step_sort)
            .execute(pool)
            .await?;
            next_step_sort += 1;
        }

        return Ok(id.to_string());
    }

    save_recipe_with_details(
        pool,
        name,
        description,
        servings,
        prep_time_minutes,
        cook_time_minutes,
        groups,
        steps,
    )
    .await
}

pub async fn list_recipe_summaries(pool: &SqlitePool) -> Result<Vec<Recipe>, sqlx::Error> {
    list_recipes(pool).await
}

pub async fn load_recipe_detail(pool: &SqlitePool, recipe_id: &str) -> Result<RecipeDetail, sqlx::Error> {
    let recipe = sqlx::query_as::<_, Recipe>(
        r#"
        SELECT id, name, description, servings, prep_time_minutes, cook_time_minutes, created_at, updated_at
        FROM recipes
        WHERE id = ?
        "#,
    )
    .bind(recipe_id)
    .fetch_one(pool)
    .await?;

    let ingredients = list_ingredients_for_recipe(pool, recipe_id).await?;
    let steps = list_steps_for_recipe(pool, recipe_id).await?;

    Ok(RecipeDetail { recipe, ingredients, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_db_creates_required_tables() {
        let pool = connect_db("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();

        let tables = sqlx::query_scalar::<_, String>(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'table'
            AND name IN ('recipes', 'tags', 'recipe_tags', 'ingredients', 'steps')
            ORDER BY name
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(tables.len(), 5);
        assert!(tables.iter().any(|name| name == "recipes"));
        assert!(tables.iter().any(|name| name == "tags"));
        assert!(tables.iter().any(|name| name == "recipe_tags"));
        assert!(tables.iter().any(|name| name == "ingredients"));
        assert!(tables.iter().any(|name| name == "steps"));
    }

    #[tokio::test]
    async fn insert_recipe_and_list_recipes_works() {
        let pool = connect_db("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();

        let recipe_id = insert_recipe(
            &pool,
            "Tomato Pasta",
            Some("Simple weeknight pasta."),
            Some(2),
            Some(10),
            Some(15),
        )
        .await
        .unwrap();

        let recipes = list_recipes(&pool).await.unwrap();

        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].id, recipe_id);
        assert_eq!(recipes[0].name, "Tomato Pasta");
        assert_eq!(recipes[0].servings, Some(2));
        assert_eq!(recipes[0].prep_time_minutes, Some(10));
        assert_eq!(recipes[0].cook_time_minutes, Some(15));
    }

    #[tokio::test]
    async fn recipe_related_data_can_be_fetched() {
        let pool = connect_db("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();

        let recipe_id = insert_recipe(
            &pool,
            "Pancakes",
            Some("Breakfast pancakes."),
            Some(4),
            Some(15),
            Some(10),
        )
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO tags (name) VALUES ('breakfast'), ('quick');",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO recipe_tags (recipe_id, tag_id) VALUES (?, 1), (?, 2);",
        )
        .bind(&recipe_id)
        .bind(&recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ingredients (recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order) VALUES (?, 'Base', '2', 'cups', 'flour', 0, 1), (?, 'Base', '1', 'cup', 'milk', 0, 2);",
        )
        .bind(&recipe_id)
        .bind(&recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO steps (recipe_id, step_number, instruction, optional, sort_order) VALUES (?, 1, 'Mix ingredients.', 0, 1), (?, 2, 'Cook on pan.', 0, 2);",
        )
        .bind(&recipe_id)
        .bind(&recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        let ingredients = list_ingredients_for_recipe(&pool, &recipe_id).await.unwrap();
        let steps = list_steps_for_recipe(&pool, &recipe_id).await.unwrap();
        let tags = list_tags_for_recipe(&pool, &recipe_id).await.unwrap();

        assert_eq!(ingredients.len(), 2);
        assert_eq!(steps.len(), 2);
        assert_eq!(tags.len(), 2);
        assert_eq!(ingredients[0].ingredient_name, "flour");
        assert_eq!(steps[1].instruction, "Cook on pan.");
        assert!(tags.iter().any(|tag| tag.name == "breakfast"));
    }

    #[tokio::test]
    async fn save_recipe_and_load_detail_round_trip() {
        let pool = connect_db("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();

        let recipe_id = save_recipe_with_details(
            &pool,
            "Pasta Primavera",
            Some("Fresh pasta with vegetables."),
            Some(2),
            Some(20),
            Some(15),
            &[
                IngredientGroupInput {
                    group_name: Some("Sauce".to_string()),
                    quantity_unit: "2 cups".to_string(),
                    description: "tomato".to_string(),
                    optional: false,
                },
                IngredientGroupInput {
                    group_name: Some("Sauce".to_string()),
                    quantity_unit: "1 tbsp".to_string(),
                    description: "olive oil".to_string(),
                    optional: true,
                },
            ],
            &[
                StepInput {
                    instruction: "Heat the pan.".to_string(),
                    optional: false,
                },
                StepInput {
                    instruction: "Add the sauce and stir.".to_string(),
                    optional: true,
                },
            ],
        )
        .await
        .unwrap();

        let detail = load_recipe_detail(&pool, &recipe_id).await.unwrap();

        assert_eq!(detail.recipe.name, "Pasta Primavera");
        assert_eq!(detail.recipe.prep_time_minutes, Some(20));
        assert_eq!(detail.ingredients.len(), 2);
        assert_eq!(detail.ingredients[0].group_name.as_deref(), Some("Sauce"));
        assert_eq!(detail.ingredients[0].ingredient_name, "tomato");
        assert_eq!(detail.steps.len(), 2);
        assert_eq!(detail.steps[1].instruction, "Add the sauce and stir.");
    }
}
