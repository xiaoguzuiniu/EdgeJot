#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod startup;
mod ui;

use std::{error::Error, path::PathBuf};

use db::Database;
use directories::BaseDirs;

fn main() -> Result<(), Box<dyn Error>> {
    let database_path = database_path()?;
    let database = Database::open(&database_path)?;
    database.ensure_starter_notes()?;
    if let Err(error) = startup::enable_for_current_user() {
        startup::report_failure(&error);
    }

    ui::run(database)?;

    Ok(())
}

fn database_path() -> Result<PathBuf, Box<dyn Error>> {
    let base_dirs = BaseDirs::new().ok_or("无法确定当前用户的数据目录")?;
    Ok(base_dirs.data_local_dir().join("Noty").join("noty.db"))
}
