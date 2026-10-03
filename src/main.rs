mod db;
mod models;
mod ui;

fn main() -> eframe::Result<()> {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let db_path = exe_dir.join("recipes.db");

    let pool = runtime.block_on(async {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
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
