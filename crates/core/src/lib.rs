pub mod booth;
pub mod embed;
pub mod settings;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
