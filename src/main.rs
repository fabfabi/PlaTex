mod db;
mod models;
mod ui;

fn main() -> eframe::Result<()> {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let pool = runtime.block_on(async {
        let pool = db::connect_db("sqlite:recipes.db").await?;
        db::init_db(&pool).await?;

        Ok::<_, sqlx::Error>(pool)
    });

    let pool = match pool {
        Ok(pool) => pool,
        Err(err) => panic!("Failed to initialize database: {err}"),
    };

    let recipe_count = runtime
        .block_on(async { db::list_recipes(&pool).await.unwrap_or_default().len() });

    eframe::run_native(
        "PlaTex",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(ui::App::new(recipe_count)))),
    )
}
