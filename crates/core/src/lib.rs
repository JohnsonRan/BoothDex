pub mod booth;
pub mod settings;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
