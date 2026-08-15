use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Db(pub Mutex<Connection>);

#[derive(Debug, Clone, Serialize)]
pub struct LastState {
    pub song_path: Option<String>,
    pub position: f64,
    pub queue: Vec<String>,
    pub volume: f64,
    pub play_mode: String,
    pub sort_mode: String,
    pub sort_dir: String,
    pub custom_sort: bool,
}

impl Db {
    pub fn new(data_dir: PathBuf) -> Self {
        let conn = Connection::open(data_dir.join("library.db"))
            .expect("failed to open library database");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS songs (
                path          TEXT PRIMARY KEY,
                title         TEXT NOT NULL DEFAULT '',
                artist        TEXT NOT NULL DEFAULT '',
                album         TEXT NOT NULL DEFAULT '',
                duration      REAL NOT NULL DEFAULT 0,
                format        TEXT NOT NULL DEFAULT '',
                track         INTEGER,
                year          INTEGER,
                genre         TEXT NOT NULL DEFAULT '',
                cover         TEXT,
                last_position REAL NOT NULL DEFAULT 0,
                updated_at    INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS player_state (
                id       INTEGER PRIMARY KEY CHECK (id = 1),
                song_path TEXT,
                position REAL NOT NULL DEFAULT 0,
                queue    TEXT NOT NULL DEFAULT '[]',
                volume   REAL NOT NULL DEFAULT 0.8
            );
            CREATE TABLE IF NOT EXISTS scan_dirs (
                path TEXT PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL DEFAULT 0,
                position   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS playlist_songs (
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                path        TEXT NOT NULL,
                position    INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (playlist_id, path)
            );
            CREATE INDEX IF NOT EXISTS idx_songs_sort_pos ON songs(sort_pos);",
        )
        .expect("failed to init database schema");
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN volume REAL NOT NULL DEFAULT 0.8", []);
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN play_mode TEXT NOT NULL DEFAULT 'order'", []);
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN window_state TEXT NOT NULL DEFAULT '{}'", []);
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN sort_mode TEXT NOT NULL DEFAULT 'title'", []);
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN sort_dir TEXT NOT NULL DEFAULT 'asc'", []);
        let _ = conn.execute("ALTER TABLE songs ADD COLUMN sort_pos INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE songs ADD COLUMN fav INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE songs DROP COLUMN play_count", []);
        let _ = conn.execute("ALTER TABLE playlists ADD COLUMN position INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE player_state ADD COLUMN custom_sort INTEGER NOT NULL DEFAULT 0", []);
        Db(Mutex::new(conn))
    }

    pub fn upsert_songs(&self, songs: &[crate::music_lib::Song]) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for s in songs {
            tx.execute(
                "INSERT INTO songs (path, title, artist, album, duration, format, track, year, genre, updated_at, sort_pos)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,
                    (SELECT COALESCE(MAX(sort_pos), 0) + 1 FROM songs))
                 ON CONFLICT(path) DO UPDATE SET
                    title=excluded.title, artist=excluded.artist, album=excluded.album,
                    duration=excluded.duration, format=excluded.format, track=excluded.track,
                    year=excluded.year, genre=excluded.genre, updated_at=excluded.updated_at",
                params![
                    s.path,
                    s.title,
                    s.artist,
                    s.album,
                    s.duration,
                    s.format,
                    s.track,
                    s.year,
                    s.genre,
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_songs(&self) -> Vec<crate::music_lib::Song> {
        let conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        // 用户从未拖过自定义顺序（custom_sort=0）时，默认按标题展示；
        // 拖过之后按 sort_pos（拖拽顺序，新歌追加在末尾）。
        let customized: i64 = conn
            .query_row(
                "SELECT COALESCE((SELECT custom_sort FROM player_state WHERE id = 1), 0)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let order = if customized > 0 {
            "ORDER BY sort_pos, title"
        } else {
            "ORDER BY title"
        };
        let mut stmt = match conn.prepare(
            &format!(
                "SELECT path, title, artist, album, duration, format, track, year, genre, fav, sort_pos FROM songs {order}"
            ),
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |r| {
            Ok(crate::music_lib::Song {
                path: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                duration: r.get(4)?,
                format: r.get(5)?,
                track: r.get(6)?,
                year: r.get(7)?,
                genre: r.get(8)?,
                fav: r.get::<_, i64>(9).unwrap_or(0) as i32,
                sort_pos: r.get::<_, i64>(10).unwrap_or(0),
            })
        });
        match rows {
            Ok(rs) => rs.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn toggle_fav(&self, path: &str) -> i32 {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "UPDATE songs SET fav = CASE WHEN fav = 0 THEN 1 ELSE 0 END WHERE path = ?1",
            params![path],
        );
        let fav: i64 = conn
            .query_row("SELECT fav FROM songs WHERE path = ?1", params![path], |r| r.get(0))
            .unwrap_or(0);
        fav as i32
    }

    pub fn save_progress(
        &self,
        song_path: &str,
        position: f64,
        queue: &[String],
        volume: f64,
        play_mode: &str,
        sort_mode: &str,
        sort_dir: &str,
    ) {
        let conn = self.0.lock().unwrap();
        let q = serde_json::to_string(queue).unwrap_or_else(|_| "[]".to_string());
        let _ = conn.execute(
            "INSERT INTO player_state (id, song_path, position, queue, volume, play_mode, sort_mode, sort_dir)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET song_path=excluded.song_path, position=excluded.position, queue=excluded.queue, volume=excluded.volume, play_mode=excluded.play_mode, sort_mode=excluded.sort_mode, sort_dir=excluded.sort_dir",
            params![song_path, position, q, volume, play_mode, sort_mode, sort_dir],
        );
    }

    pub fn get_last_state(&self) -> Option<LastState> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT song_path, position, queue, volume, COALESCE(play_mode, 'order'), COALESCE(sort_mode, 'custom'), COALESCE(sort_dir, 'asc'), COALESCE(custom_sort, 0) FROM player_state WHERE id = 1")
            .ok()?;
        let mut rows = stmt.query([]).ok()?;
        let row = rows.next().ok().flatten()?;
        let path: Option<String> = row.get(0).ok()?;
        let position: f64 = row.get(1).unwrap_or(0.0);
        let queue_json: String = row.get(2).unwrap_or_else(|_| "[]".to_string());
        let queue: Vec<String> = serde_json::from_str(&queue_json).unwrap_or_default();
        let volume: f64 = row.get(3).unwrap_or(0.8);
        let play_mode: String = row.get(4).unwrap_or_else(|_| "order".to_string());
        let sort_mode: String = row.get(5).unwrap_or_else(|_| "custom".to_string());
        let sort_dir: String = row.get(6).unwrap_or_else(|_| "asc".to_string());
        let custom_sort: bool = row.get::<_, i64>(7).unwrap_or(0) != 0;
        Some(LastState {
            song_path: path,
            position,
            queue,
            volume,
            play_mode,
            sort_mode,
            sort_dir,
            custom_sort,
        })
    }

    pub fn get_scan_dirs(&self) -> Vec<String> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT path FROM scan_dirs ORDER BY path") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |r| r.get::<_, String>(0));
        match rows {
            Ok(rs) => rs.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn get_window_state(&self) -> String {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT window_state FROM player_state WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "{}".to_string())
    }

    pub fn set_window_state(&self, state: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO player_state (id, window_state) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET window_state = excluded.window_state",
            params![state],
        );
    }

    pub fn add_scan_dir(&self, path: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("INSERT OR IGNORE INTO scan_dirs (path) VALUES (?1)", params![path]);
    }

    pub fn remove_scan_dir(&self, path: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM scan_dirs WHERE path = ?1", params![path]);
        let _ = conn.execute("DELETE FROM songs WHERE path LIKE ?1", params![format!("{path}\\%")]);
    }

    pub fn create_playlist(&self, name: &str) -> Result<i64, String> {
        let conn = self.0.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO playlists (name, created_at, position) VALUES (?1, ?2,
             (SELECT COALESCE(MAX(position), 0) + 1 FROM playlists))",
            params![name, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<(), String> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE playlists SET name = ?2 WHERE id = ?1",
            params![id, name],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM playlists WHERE id = ?1", params![id]);
    }

    pub fn list_playlists(&self) -> Vec<PlaylistInfo> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT p.id, p.name, (SELECT COUNT(*) FROM playlist_songs ps WHERE ps.playlist_id = p.id)
             FROM playlists p ORDER BY p.position, p.created_at",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |r| {
            Ok(PlaylistInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get(2)?,
            })
        });
        match rows {
            Ok(rs) => rs.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn reorder_playlists(&self, ids: &[i64]) {
        let mut conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };
        let mut stmt = match tx.prepare("UPDATE playlists SET position = ?2 WHERE id = ?1") {
            Ok(s) => s,
            Err(_) => return,
        };
        for (i, id) in ids.iter().enumerate() {
            let _ = stmt.execute(params![id, i as i64]);
        }
        drop(stmt);
        let _ = tx.commit();
    }

    pub fn add_to_playlist(&self, id: i64, path: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO playlist_songs (playlist_id, path, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_songs WHERE playlist_id = ?1))",
            params![id, path],
        );
    }

    pub fn add_songs_to_playlist(&self, id: i64, paths: &[String]) {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(
            "INSERT OR IGNORE INTO playlist_songs (playlist_id, path, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_songs WHERE playlist_id = ?1))",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        for p in paths {
            let _ = stmt.execute(params![id, p]);
        }
    }

    pub fn reorder_playlist_songs(&self, id: i64, paths: &[String]) {
        let mut conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };
        let mut stmt = match tx.prepare(
            "UPDATE playlist_songs SET position = ?3 WHERE playlist_id = ?1 AND path = ?2",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        for (i, p) in paths.iter().enumerate() {
            let _ = stmt.execute(params![id, p, i as i64]);
        }
        drop(stmt);
        let _ = tx.commit();
    }

    pub fn reorder_songs(&self, paths: &[String]) {
        let mut conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };
        let mut stmt = match tx.prepare("UPDATE songs SET sort_pos = ?2 WHERE path = ?1") {
            Ok(s) => s,
            Err(_) => return,
        };
        for (i, p) in paths.iter().enumerate() {
            let _ = stmt.execute(params![p, i as i64]);
        }
        drop(stmt);
        // 一旦手动拖过，自定义顺序生效（此后按 sort_pos 展示，新歌追加在末尾）
        let _ = tx.execute(
            "INSERT INTO player_state (id, custom_sort) VALUES (1, 1)
             ON CONFLICT(id) DO UPDATE SET custom_sort = 1",
            [],
        );
        let _ = tx.commit();
    }

    pub fn remove_from_playlist(&self, id: i64, path: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "DELETE FROM playlist_songs WHERE playlist_id = ?1 AND path = ?2",
            params![id, path],
        );
    }

    pub fn get_playlist_songs(&self, id: i64) -> Vec<crate::music_lib::Song> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT s.path, s.title, s.artist, s.album, s.duration, s.format, s.track, s.year, s.genre, s.fav, s.sort_pos
             FROM playlist_songs ps JOIN songs s ON s.path = ps.path
             WHERE ps.playlist_id = ?1 ORDER BY ps.position",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([id], |r| {
            Ok(crate::music_lib::Song {
                path: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                duration: r.get(4)?,
                format: r.get(5)?,
                track: r.get(6)?,
                year: r.get(7)?,
                genre: r.get(8)?,
                fav: r.get::<_, i64>(9).unwrap_or(0) as i32,
                sort_pos: r.get::<_, i64>(10).unwrap_or(0),
            })
        });
        match rows {
            Ok(rs) => rs.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistInfo {
    pub id: i64,
    pub name: String,
    pub count: i64,
}
