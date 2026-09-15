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
    pub id: String,
    pub title: String,
    pub content: String,
    pub color: String,
    pub is_pinned: bool,
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
        migrate_schema(&connection)?;

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
            "SELECT id, title, content, color, is_pinned
             FROM notes
             ORDER BY is_pinned DESC, updated_at DESC, rowid ASC",
        )?;
        statement
            .query_map([], |row| {
                Ok(NoteSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    color: row.get(3)?,
                    is_pinned: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect()
    }

    pub fn update_note(&self, id: &str, title: &str, content: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE notes SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
            (title, content, unix_timestamp(), id),
        )?;
        Ok(())
    }

    pub fn create_note(&self) -> rusqlite::Result<()> {
        let colors = ["yellow", "green", "purple"];
        let note_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        let color = colors[note_count as usize % colors.len()];
        self.insert_note("新便签", color, unix_timestamp())
    }

    pub fn set_note_pinned(&self, id: &str, is_pinned: bool) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE notes SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
            (i64::from(is_pinned), unix_timestamp(), id),
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM notes WHERE id = ?1", [id])?;
        Ok(())
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

fn migrate_schema(connection: &Connection) -> rusqlite::Result<()> {
    let has_is_pinned: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('notes') WHERE name = 'is_pinned')",
        [],
        |row| row.get(0),
    )?;
    if !has_is_pinned {
        connection.execute(
            "ALTER TABLE notes ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0
             CHECK (is_pinned IN (0, 1))",
            [],
        )?;
    }
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_notes_pinned_updated
         ON notes(is_pinned DESC, updated_at DESC);",
    )
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
    use rusqlite::Connection;

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
        assert!(notes.iter().all(|note| !note.is_pinned));

        database.create_note().expect("应能新增便签");
        let notes = database.list_notes().expect("新增后应能刷新便签列表");
        assert_eq!(notes.len(), 3);
        assert!(notes.iter().any(|note| note.title == "新便签"));

        let edited_id = notes[0].id.clone();
        database
            .update_note(&edited_id, "编辑后的标题", "编辑后的正文")
            .expect("应能保存便签内容");
        let edited = database
            .list_notes()
            .expect("应能读取编辑后的便签")
            .into_iter()
            .find(|note| note.id == edited_id)
            .expect("编辑后的便签应存在");
        assert_eq!(edited.title, "编辑后的标题");
        assert_eq!(edited.content, "编辑后的正文");

        database
            .set_note_pinned(&edited_id, true)
            .expect("应能置顶便签");
        let pinned_notes = database.list_notes().expect("应能读取置顶后的列表");
        assert_eq!(pinned_notes[0].id, edited_id);
        assert!(pinned_notes[0].is_pinned);
        database
            .set_note_pinned(&edited_id, false)
            .expect("应能取消置顶");

        for _ in 0..8 {
            database.create_note().expect("应能创建超过九张便签");
        }
        let notes = database.list_notes().expect("应能读取全部便签");
        assert_eq!(notes.len(), 11);

        database.delete_note(&edited_id).expect("应能删除便签");
        let notes = database.list_notes().expect("删除后应能刷新便签列表");
        assert_eq!(notes.len(), 10);
        assert!(notes.iter().all(|note| note.id != edited_id));

        drop(database);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn migrates_legacy_notes_with_pin_state() {
        let path = std::env::temp_dir().join(format!("noty-legacy-test-{}.db", std::process::id()));
        let connection = Connection::open(&path).expect("应能创建旧版测试数据库");
        connection
            .execute_batch(
                "CREATE TABLE notes (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL DEFAULT '',
                    content TEXT NOT NULL DEFAULT '',
                    color TEXT NOT NULL DEFAULT 'yellow',
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );",
            )
            .expect("应能建立旧版表结构");
        drop(connection);

        let database = Database::open(&path).expect("应能迁移旧版数据库");
        let has_pin_column: bool = database
            .connection()
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('notes') WHERE name = 'is_pinned'
                )",
                [],
                |row| row.get(0),
            )
            .expect("应能读取迁移后的表结构");
        assert!(has_pin_column);

        drop(database);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }
}
