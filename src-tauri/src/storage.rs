use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const DB_FILE_NAME: &str = "snipjar.db";

#[derive(Clone)]
pub struct DatabaseState {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: String,
    pub key: String,
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryInput {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResult {
    pub mode: PasteMode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteMode {
    Pasted,
    CopiedOnly,
}

#[derive(Debug)]
pub enum StorageError {
    ResolvePath(tauri::Error),
    CreateDirectory(std::io::Error),
    OpenDatabase(rusqlite::Error),
    Migrate(rusqlite::Error),
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResolvePath(err) => write!(f, "failed to resolve app local data path: {err}"),
            Self::CreateDirectory(err) => write!(f, "failed to create app local data directory: {err}"),
            Self::OpenDatabase(err) => write!(f, "failed to open sqlite database: {err}"),
            Self::Migrate(err) => write!(f, "failed to apply sqlite migrations: {err}"),
        }
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug)]
pub enum DataError {
    Validation(String),
    DuplicateKey,
    NotFound(String),
    TimeFormatting(time::error::Format),
    Clipboard(arboard::Error),
    Sql(rusqlite::Error),
}

impl Display for DataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{message}"),
            Self::DuplicateKey => write!(f, "key must be unique"),
            Self::NotFound(id) => write!(f, "entry not found for id: {id}"),
            Self::TimeFormatting(err) => write!(f, "failed to format timestamp: {err}"),
            Self::Clipboard(err) => write!(f, "clipboard operation failed: {err}"),
            Self::Sql(err) => write!(f, "database operation failed: {err}"),
        }
    }
}

impl std::error::Error for DataError {}

pub fn initialize(app_handle: &tauri::AppHandle) -> Result<DatabaseState, StorageError> {
    let app_local_dir = app_handle
        .path()
        .app_local_data_dir()
        .map_err(StorageError::ResolvePath)?;

    fs::create_dir_all(&app_local_dir).map_err(StorageError::CreateDirectory)?;

    let db_path = app_local_dir.join(DB_FILE_NAME);
    init_at(&db_path)?;

    Ok(DatabaseState { path: db_path })
}

pub fn init_at(db_path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open(db_path).map_err(StorageError::OpenDatabase)?;
    apply_migrations(&connection).map_err(StorageError::Migrate)?;
    Ok(())
}

pub fn create_entry(state: &DatabaseState, input: EntryInput) -> Result<Entry, DataError> {
    let normalized = NormalizedEntryInput::new(input)?;
    let entry_id = Uuid::new_v4().to_string();
    let now = now_rfc3339()?;

    let mut connection = open_connection(state)?;
    let tx = connection.transaction().map_err(DataError::Sql)?;

    tx.execute(
        "INSERT INTO entries (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![entry_id, normalized.key, normalized.value, now, now],
    )
    .map_err(map_write_error)?;

    insert_tags_for_entry(&tx, &entry_id, &normalized.tags)?;
    tx.commit().map_err(DataError::Sql)?;

    fetch_entry_by_id(&connection, &entry_id)?
        .ok_or_else(|| DataError::NotFound(entry_id))
}

pub fn update_entry(state: &DatabaseState, id: &str, input: EntryInput) -> Result<Entry, DataError> {
    let normalized = NormalizedEntryInput::new(input)?;
    let now = now_rfc3339()?;

    let mut connection = open_connection(state)?;
    let tx = connection.transaction().map_err(DataError::Sql)?;

    let updated_rows = tx
        .execute(
            "UPDATE entries SET key = ?1, value = ?2, updated_at = ?3 WHERE id = ?4",
            params![normalized.key, normalized.value, now, id],
        )
        .map_err(map_write_error)?;
    if updated_rows == 0 {
        return Err(DataError::NotFound(id.to_string()));
    }

    tx.execute("DELETE FROM entry_tags WHERE entry_id = ?1", params![id])
        .map_err(DataError::Sql)?;
    insert_tags_for_entry(&tx, id, &normalized.tags)?;
    tx.commit().map_err(DataError::Sql)?;

    fetch_entry_by_id(&connection, id)?
        .ok_or_else(|| DataError::NotFound(id.to_string()))
}

pub fn get_entry(state: &DatabaseState, id: &str) -> Result<Entry, DataError> {
    let connection = open_connection(state)?;
    fetch_entry_by_id(&connection, id)?
        .ok_or_else(|| DataError::NotFound(id.to_string()))
}

pub fn delete_entry(state: &DatabaseState, id: &str) -> Result<(), DataError> {
    let connection = open_connection(state)?;
    let deleted_rows = connection
        .execute("DELETE FROM entries WHERE id = ?1", params![id])
        .map_err(DataError::Sql)?;
    if deleted_rows == 0 {
        return Err(DataError::NotFound(id.to_string()));
    }
    Ok(())
}

pub fn get_recent_entries(state: &DatabaseState) -> Result<Vec<EntrySummary>, DataError> {
    let connection = open_connection(state)?;
    fetch_recent_summaries(&connection)
}

pub fn search_entries(state: &DatabaseState, query: &str) -> Result<Vec<EntrySummary>, DataError> {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return get_recent_entries(state);
    }

    let entries = get_recent_entries(state)?;
    let mut ranked = entries
        .into_iter()
        .filter_map(|entry| {
            let key_lower = entry.key.to_lowercase();
            let match_class = if key_lower.starts_with(&normalized_query) {
                Some(0)
            } else if key_lower.contains(&normalized_query) {
                Some(1)
            } else if entry
                .tags
                .iter()
                .any(|tag| tag.contains(&normalized_query))
            {
                Some(2)
            } else {
                None
            };

            match_class.map(|class| RankedSummary { class, entry })
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        left.class
            .cmp(&right.class)
            .then_with(|| right.entry.updated_at.cmp(&left.entry.updated_at))
            .then_with(|| left.entry.key.cmp(&right.entry.key))
            .then_with(|| left.entry.id.cmp(&right.entry.id))
    });

    Ok(ranked.into_iter().map(|item| item.entry).collect())
}

pub fn copy_entry(state: &DatabaseState, id: &str) -> Result<PasteResult, DataError> {
    let connection = open_connection(state)?;
    let entry = fetch_entry_by_id(&connection, id)?
        .ok_or_else(|| DataError::NotFound(id.to_string()))?;

    copy_value_to_clipboard(&entry.value)?;

    Ok(PasteResult {
        mode: PasteMode::CopiedOnly,
        message: format!("Copied \"{}\" to clipboard.", entry.key),
    })
}

pub fn paste_entry(state: &DatabaseState, id: &str) -> Result<PasteResult, DataError> {
    let connection = open_connection(state)?;
    let entry = fetch_entry_by_id(&connection, id)?
        .ok_or_else(|| DataError::NotFound(id.to_string()))?;

    copy_value_to_clipboard(&entry.value)?;
    match attempt_cmd_v_paste() {
        Ok(_) => Ok(PasteResult {
            mode: PasteMode::Pasted,
            message: format!("Pasted \"{}\".", entry.key),
        }),
        Err(reason) => Ok(PasteResult {
            mode: PasteMode::CopiedOnly,
            message: format!(
                "Copied \"{}\" to clipboard. Auto-paste unavailable: {reason}",
                entry.key
            ),
        }),
    }
}

fn apply_migrations(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS entries (
            id TEXT PRIMARY KEY NOT NULL,
            key TEXT NOT NULL UNIQUE CHECK (length(trim(key)) > 0),
            value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entry_tags (
            entry_id TEXT NOT NULL,
            tag TEXT NOT NULL CHECK (length(trim(tag)) > 0 AND tag = lower(tag)),
            PRIMARY KEY (entry_id, tag),
            FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_entry_tags_tag ON entry_tags(tag);
        "#,
    )
}

fn open_connection(state: &DatabaseState) -> Result<Connection, DataError> {
    let connection = Connection::open(&state.path).map_err(DataError::Sql)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(DataError::Sql)?;
    Ok(connection)
}

fn now_rfc3339() -> Result<String, DataError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(DataError::TimeFormatting)
}

fn copy_value_to_clipboard(value: &str) -> Result<(), DataError> {
    let mut clipboard = Clipboard::new().map_err(DataError::Clipboard)?;
    clipboard
        .set_text(value.to_string())
        .map_err(DataError::Clipboard)
}

fn attempt_cmd_v_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|err| err.to_string())?;
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|err| err.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|err| err.to_string())?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|err| err.to_string())?;

    thread::sleep(Duration::from_millis(40));
    Ok(())
}

fn map_write_error(err: rusqlite::Error) -> DataError {
    match &err {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("UNIQUE constraint failed: entries.key") =>
        {
            DataError::DuplicateKey
        }
        _ => DataError::Sql(err),
    }
}

fn fetch_recent_summaries(connection: &Connection) -> Result<Vec<EntrySummary>, DataError> {
    let mut statement = connection
        .prepare("SELECT id, key, updated_at FROM entries ORDER BY updated_at DESC")
        .map_err(DataError::Sql)?;
    let mut rows = statement.query([]).map_err(DataError::Sql)?;
    let mut entries = Vec::new();

    while let Some(row) = rows.next().map_err(DataError::Sql)? {
        let id: String = row.get(0).map_err(DataError::Sql)?;
        let key: String = row.get(1).map_err(DataError::Sql)?;
        let updated_at: String = row.get(2).map_err(DataError::Sql)?;
        let tags = fetch_tags_for_entry(connection, &id).map_err(DataError::Sql)?;
        entries.push(EntrySummary {
            id,
            key,
            tags,
            updated_at,
        });
    }

    Ok(entries)
}

fn fetch_entry_by_id(connection: &Connection, id: &str) -> Result<Option<Entry>, DataError> {
    let row = connection
        .query_row(
            "SELECT id, key, value, created_at, updated_at FROM entries WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(DataError::Sql)?;

    row.map(|(id, key, value, created_at, updated_at)| {
        let tags = fetch_tags_for_entry(connection, &id).map_err(DataError::Sql)?;
        Ok(Entry {
            id,
            key,
            value,
            tags,
            created_at,
            updated_at,
        })
    })
    .transpose()
}

fn fetch_tags_for_entry(connection: &Connection, entry_id: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = connection.prepare("SELECT tag FROM entry_tags WHERE entry_id = ?1 ORDER BY tag ASC")?;
    let mut rows = statement.query(params![entry_id])?;
    let mut tags = Vec::new();

    while let Some(row) = rows.next()? {
        let tag: String = row.get(0)?;
        tags.push(tag);
    }

    Ok(tags)
}

fn insert_tags_for_entry(
    tx: &rusqlite::Transaction<'_>,
    entry_id: &str,
    tags: &[String],
) -> Result<(), DataError> {
    for tag in tags {
        tx.execute(
            "INSERT INTO entry_tags (entry_id, tag) VALUES (?1, ?2)",
            params![entry_id, tag],
        )
        .map_err(DataError::Sql)?;
    }
    Ok(())
}

struct RankedSummary {
    class: i32,
    entry: EntrySummary,
}

struct NormalizedEntryInput {
    key: String,
    value: String,
    tags: Vec<String>,
}

impl NormalizedEntryInput {
    fn new(input: EntryInput) -> Result<Self, DataError> {
        let key = input.key.trim().to_string();
        if key.is_empty() {
            return Err(DataError::Validation("key must be non-empty".to_string()));
        }

        if input.value.trim().is_empty() {
            return Err(DataError::Validation("value must be non-empty".to_string()));
        }

        Ok(Self {
            key,
            value: input.value,
            tags: normalize_tags(input.tags),
        })
    }
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized_tags = Vec::new();

    for raw_tag in tags {
        let normalized = raw_tag.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }

        if seen.insert(normalized.clone()) {
            normalized_tags.push(normalized);
        }
    }

    normalized_tags
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rusqlite::Connection;
    use uuid::Uuid;

    use super::{
        apply_migrations, create_entry, delete_entry, get_entry, get_recent_entries, init_at,
        search_entries, update_entry, DataError, DatabaseState, EntryInput,
    };

    struct TestDb {
        state: DatabaseState,
        path: PathBuf,
    }

    impl TestDb {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("snipjar-storage-test-{}.db", Uuid::new_v4()));
            init_at(&path).expect("initialize test database");
            Self {
                state: DatabaseState { path: path.clone() },
                path,
            }
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn enforces_non_empty_unique_key_and_deduped_tags() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        apply_migrations(&connection).expect("schema migration");

        connection
            .execute(
                "INSERT INTO entries (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                ("1", "email", "test@example.com", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            )
            .expect("insert first entry");

        let duplicate_key_error = connection
            .execute(
                "INSERT INTO entries (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                ("2", "email", "another@example.com", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            )
            .expect_err("duplicate key must fail");
        assert_eq!(duplicate_key_error.sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));

        let empty_key_error = connection
            .execute(
                "INSERT INTO entries (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                ("3", "", "empty key", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            )
            .expect_err("empty key must fail");
        assert_eq!(empty_key_error.sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));

        connection
            .execute("INSERT INTO entry_tags (entry_id, tag) VALUES (?1, ?2)", ("1", "work"))
            .expect("insert first tag");

        let duplicate_tag_error = connection
            .execute("INSERT INTO entry_tags (entry_id, tag) VALUES (?1, ?2)", ("1", "work"))
            .expect_err("duplicate tag must fail");
        assert_eq!(duplicate_tag_error.sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));
    }

    #[test]
    fn enforces_lowercase_tags() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        apply_migrations(&connection).expect("schema migration");

        connection
            .execute(
                "INSERT INTO entries (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                ("1", "project", "snipjar", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            )
            .expect("insert entry");

        let uppercase_tag_error = connection
            .execute("INSERT INTO entry_tags (entry_id, tag) VALUES (?1, ?2)", ("1", "Work"))
            .expect_err("uppercase tag must fail");
        assert_eq!(uppercase_tag_error.sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));
    }

    #[test]
    fn create_update_search_recent_and_delete_flow() {
        let db = TestDb::new();

        let alpha = create_entry(
            &db.state,
            EntryInput {
                key: "email".to_string(),
                value: "alpha@example.com".to_string(),
                tags: vec![" Work ".to_string(), "WORK".to_string(), "".to_string()],
            },
        )
        .expect("create alpha");
        let beta = create_entry(
            &db.state,
            EntryInput {
                key: "my-email-template".to_string(),
                value: "template".to_string(),
                tags: vec!["notes".to_string()],
            },
        )
        .expect("create beta");
        let gamma = create_entry(
            &db.state,
            EntryInput {
                key: "project".to_string(),
                value: "gamma".to_string(),
                tags: vec!["email".to_string()],
            },
        )
        .expect("create gamma");

        assert_eq!(alpha.tags, vec!["work".to_string()]);

        let loaded_beta = get_entry(&db.state, &beta.id).expect("load beta");
        assert_eq!(loaded_beta.value, "template");

        let ranked = search_entries(&db.state, "email").expect("search by email");
        assert_eq!(ranked[0].id, alpha.id, "prefix match should rank first");
        assert_eq!(ranked[1].id, beta.id, "substring key match should rank second");
        assert_eq!(ranked[2].id, gamma.id, "tag match should rank after key matches");

        let updated = update_entry(
            &db.state,
            &beta.id,
            EntryInput {
                key: "snippet".to_string(),
                value: "updated-value".to_string(),
                tags: vec!["Email".to_string(), "team".to_string(), "team".to_string()],
            },
        )
        .expect("update beta");

        assert_eq!(updated.tags, vec!["email".to_string(), "team".to_string()]);

        let recent = get_recent_entries(&db.state).expect("get recent entries");
        assert_eq!(recent[0].id, beta.id, "recent entries should prioritize updated_at");

        delete_entry(&db.state, &gamma.id).expect("delete gamma");
        let after_delete = get_recent_entries(&db.state).expect("entries after delete");
        assert_eq!(after_delete.len(), 2);
        assert!(!after_delete.iter().any(|item| item.id == gamma.id));
    }

    #[test]
    fn create_and_update_validate_input_and_unique_key() {
        let db = TestDb::new();

        let created = create_entry(
            &db.state,
            EntryInput {
                key: "email".to_string(),
                value: "alpha@example.com".to_string(),
                tags: vec![],
            },
        )
        .expect("create entry");

        let duplicate = create_entry(
            &db.state,
            EntryInput {
                key: " email ".to_string(),
                value: "another@example.com".to_string(),
                tags: vec![],
            },
        )
        .expect_err("duplicate key should fail");
        assert!(matches!(duplicate, DataError::DuplicateKey));

        let empty_key = create_entry(
            &db.state,
            EntryInput {
                key: "  ".to_string(),
                value: "v".to_string(),
                tags: vec![],
            },
        )
        .expect_err("empty key should fail");
        assert!(matches!(empty_key, DataError::Validation(_)));

        let empty_value = update_entry(
            &db.state,
            &created.id,
            EntryInput {
                key: "new".to_string(),
                value: "   ".to_string(),
                tags: vec![],
            },
        )
        .expect_err("empty value should fail");
        assert!(matches!(empty_value, DataError::Validation(_)));
    }
}
