use sqlx::SqlitePool;

use crate::models::{Ingredient, Recipe, Step, Tag};

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
    )
    .bind(recipe_id)
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
    )
    .bind(recipe_id)
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
        .bind(recipe_id)
        .bind(recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ingredients (recipe_id, group_name, quantity, unit, ingredient_name, optional, sort_order) VALUES (?, 'Base', '2', 'cups', 'flour', 0, 1), (?, 'Base', '1', 'cup', 'milk', 0, 2);",
        )
        .bind(recipe_id)
        .bind(recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO steps (recipe_id, step_number, instruction, optional, sort_order) VALUES (?, 1, 'Mix ingredients.', 0, 1), (?, 2, 'Cook on pan.', 0, 2);",
        )
        .bind(recipe_id)
        .bind(recipe_id)
        .execute(&pool)
        .await
        .unwrap();

        let ingredients = list_ingredients_for_recipe(&pool, recipe_id).await.unwrap();
        let steps = list_steps_for_recipe(&pool, recipe_id).await.unwrap();
        let tags = list_tags_for_recipe(&pool, recipe_id).await.unwrap();

        assert_eq!(ingredients.len(), 2);
        assert_eq!(steps.len(), 2);
        assert_eq!(tags.len(), 2);
        assert_eq!(ingredients[0].ingredient_name, "flour");
        assert_eq!(steps[1].instruction, "Cook on pan.");
        assert!(tags.iter().any(|tag| tag.name == "breakfast"));
    }
}
