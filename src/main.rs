mod db;
mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = db::connect_db("sqlite:recipes.db").await?;
    db::init_db(&pool).await?;

    let recipe_id = db::insert_recipe(
        &pool,
        "Tomato Pasta",
        Some("A simple weeknight pasta."),
        Some(2),
        Some(10),
        Some(15),
    )
    .await?;

    let recipes = db::list_recipes(&pool).await?;
    println!("Recipes found: {}", recipes.len());
    println!("Inserted recipe id: {}", recipe_id);

    Ok(())
}
