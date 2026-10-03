mod db;
mod export;
mod models;
mod ui;

use std::sync::Arc;

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
    let pool = Arc::new(pool);

    let recipes = runtime
        .block_on(async { db::list_recipe_summaries(&pool).await.unwrap_or_default() });

    eframe::run_native(
        "PlaTex",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([900.0, 675.0])
                .with_min_inner_size([700.0, 450.0])
                .with_resizable(true),
            ..Default::default()
        },
        Box::new(move |_cc| Ok(Box::new(ui::App::new(pool.clone(), recipes.clone())))),
    )
}
