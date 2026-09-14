use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
    id          TEXT PRIMARY KEY NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    content     TEXT NOT NULL DEFAULT '',
    color       TEXT NOT NULL DEFAULT 'yellow',
    is_pinned   INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_updated_at
ON notes(updated_at DESC);
"#;

pub struct Database {
    connection: Connection,
}

#[derive(Clone, Debug)]
pub struct NoteSummary {
    pub title: String,
    pub color: String,
}

impl Database {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        }

        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(SCHEMA)?;

        Ok(Self { connection })
    }

    #[allow(dead_code)]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn ensure_starter_notes(&self) -> rusqlite::Result<()> {
        let note_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        if note_count == 0 {
            let now = unix_timestamp();
            self.insert_note("开发任务总结...", "purple", now + 1)?;
            self.insert_note("转正答辩", "green", now)?;
        }
        Ok(())
    }

    pub fn list_notes(&self) -> rusqlite::Result<Vec<NoteSummary>> {
        let mut statement = self.connection.prepare(
            "SELECT title, color FROM notes ORDER BY updated_at DESC, rowid ASC LIMIT 9",
        )?;
        statement
            .query_map([], |row| {
                Ok(NoteSummary {
                    title: row.get(0)?,
                    color: row.get(1)?,
                })
            })?
            .collect()
    }

    pub fn create_note(&self) -> rusqlite::Result<()> {
        let colors = ["yellow", "green", "purple"];
        let note_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        let color = colors[note_count as usize % colors.len()];
        self.insert_note("新便签", color, unix_timestamp())
    }

    fn insert_note(&self, title: &str, color: &str, timestamp: i64) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO notes (id, title, content, color, is_pinned, created_at, updated_at)
             VALUES (lower(hex(randomblob(16))), ?1, '', ?2, 0, ?3, ?3)",
            (title, color, timestamp),
        )?;
        Ok(())
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::Database;

    #[test]
    fn creates_notes_schema() {
        let path = std::env::temp_dir().join(format!("noty-test-{}.db", std::process::id()));
        let database = Database::open(&path).expect("数据库应能创建");

        let table_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
                [],
                |row| row.get(0),
            )
            .expect("应能查询数据库结构");

        assert_eq!(table_count, 1);

        database.ensure_starter_notes().expect("应能创建初始便签");
        let notes = database.list_notes().expect("应能读取便签列表");
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].title, "开发任务总结...");

        database.create_note().expect("应能新增便签");
        let notes = database.list_notes().expect("新增后应能刷新便签列表");
        assert_eq!(notes.len(), 3);
        assert!(notes.iter().any(|note| note.title == "新便签"));

        drop(database);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }
}
