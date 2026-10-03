use sqlx::SqlitePool;

use crate::models::{Ingredient, Recipe, RecipeTag, Step, Tag};

pub async fn connect_db(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePool::connect(database_url).await
}

pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recipes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
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
            recipe_id INTEGER NOT NULL,
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
            recipe_id INTEGER NOT NULL,
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
            recipe_id INTEGER NOT NULL,
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

pub async fn list_ingredients_for_recipe(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<Ingredient>, sqlx::Error> {
    let ingredients = sqlx::query_as::<_, Ingredient>(
        r#"
        SELECT id, recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order
        FROM ingredients
        WHERE recipe_id = ?
        ORDER BY sort_order
        "#,
        recipe_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(ingredients)
}

pub async fn list_steps_for_recipe(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<Step>, sqlx::Error> {
    let steps = sqlx::query_as::<_, Step>(
        r#"
        SELECT id, recipe_id, step_number, instruction, optional, sort_order
        FROM steps
        WHERE recipe_id = ?
        ORDER BY sort_order
        "#,
        recipe_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(steps)
}

pub async fn list_tags_for_recipe(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<Tag>, sqlx::Error> {
    let tags = sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.id, t.name
        FROM tags t
        INNER JOIN recipe_tags rt ON rt.tag_id = t.id
        WHERE rt.recipe_id = ?
        ORDER BY t.name
        "#,
        recipe_id,
    )
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
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO recipes (name, description, servings, prep_time_minutes, cook_time_minutes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'))
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(servings)
    .bind(prep_time_minutes)
    .bind(cook_time_minutes)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}
