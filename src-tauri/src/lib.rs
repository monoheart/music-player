mod db;
mod music_lib;
mod player;

use db::{Db, LastState, PlaylistInfo};
use player::StreamMsg;
use tauri::{AppHandle, Manager, State};
use tauri::ipc::Channel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            app.manage(Db::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_library,
            save_progress,
            get_last_state,
            get_songs,
            get_cover,
            get_lyrics,
            get_scan_dirs,
            get_file_info,
            log_error,
            log_info,
            add_scan_dir,
            remove_scan_dir,
            play_stream,
            seek_stream,
            ack_chunks,
            set_paused,
            toggle_fav,
            create_playlist,
            rename_playlist,
            delete_playlist,
            list_playlists,
            add_to_playlist,
            remove_from_playlist,
            get_playlist_songs,
            reorder_playlists,
            add_songs_to_playlist,
            reorder_playlist_songs,
            reorder_songs,
            get_window_state,
            set_window_state,
            reveal_in_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn scan_library(dir: String, db: State<'_, Db>) -> Result<Vec<music_lib::Song>, String> {
    let songs = music_lib::scan_dir(&dir);
    db.upsert_songs(&songs).map_err(|e| e.to_string())?;
    Ok(songs)
}

#[tauri::command]
fn save_progress(
    song_path: String,
    position: f64,
    queue: Vec<String>,
    volume: f64,
    play_mode: String,
    sort_mode: String,
    sort_dir: String,
    db: State<'_, Db>,
) -> Result<(), String> {
    db.save_progress(&song_path, position, &queue, volume, &play_mode, &sort_mode, &sort_dir);
    Ok(())
}

#[tauri::command]
fn toggle_fav(path: String, db: State<'_, Db>) -> Result<i32, String> {
    Ok(db.toggle_fav(&path))
}

#[tauri::command]
fn get_file_info(path: String) -> Result<Option<music_lib::FileInfo>, String> {
    Ok(music_lib::file_info(&path))
}

#[tauri::command]
fn create_playlist(name: String, db: State<'_, Db>) -> Result<i64, String> {
    db.create_playlist(&name)
}

#[tauri::command]
fn rename_playlist(id: i64, name: String, db: State<'_, Db>) -> Result<(), String> {
    db.rename_playlist(id, &name)
}

#[tauri::command]
fn delete_playlist(id: i64, db: State<'_, Db>) -> Result<(), String> {
    db.delete_playlist(id);
    Ok(())
}

#[tauri::command]
fn list_playlists(db: State<'_, Db>) -> Result<Vec<PlaylistInfo>, String> {
    Ok(db.list_playlists())
}

#[tauri::command]
fn add_to_playlist(id: i64, path: String, db: State<'_, Db>) -> Result<(), String> {
    db.add_to_playlist(id, &path);
    Ok(())
}

#[tauri::command]
fn remove_from_playlist(id: i64, path: String, db: State<'_, Db>) -> Result<(), String> {
    db.remove_from_playlist(id, &path);
    Ok(())
}

#[tauri::command]
fn get_playlist_songs(id: i64, db: State<'_, Db>) -> Result<Vec<music_lib::Song>, String> {
    Ok(db.get_playlist_songs(id))
}

#[tauri::command]
fn reorder_playlists(ids: Vec<i64>, db: State<'_, Db>) -> Result<(), String> {
    db.reorder_playlists(&ids);
    Ok(())
}

#[tauri::command]
fn add_songs_to_playlist(id: i64, paths: Vec<String>, db: State<'_, Db>) -> Result<(), String> {
    db.add_songs_to_playlist(id, &paths);
    Ok(())
}

#[tauri::command]
fn reorder_playlist_songs(id: i64, paths: Vec<String>, db: State<'_, Db>) -> Result<(), String> {
    db.reorder_playlist_songs(id, &paths);
    Ok(())
}

#[tauri::command]
fn reorder_songs(paths: Vec<String>, db: State<'_, Db>) -> Result<(), String> {
    db.reorder_songs(&paths);
    Ok(())
}

#[tauri::command]
fn get_window_state(db: State<'_, Db>) -> Result<String, String> {
    Ok(db.get_window_state())
}

#[tauri::command]
fn set_window_state(state: String, db: State<'_, Db>) -> Result<(), String> {
    db.set_window_state(&state);
    Ok(())
}

#[tauri::command]
fn reveal_in_folder(path: String) -> Result<(), String> {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn log_error(message: String) {
    eprintln!("[webview-error] {message}");
}

#[tauri::command]
fn log_info(message: String) {
    eprintln!("[audio] {message}");
}

#[tauri::command]
fn get_scan_dirs(db: State<'_, Db>) -> Result<Vec<String>, String> {
    Ok(db.get_scan_dirs())
}

#[tauri::command]
fn add_scan_dir(path: String, db: State<'_, Db>) -> Result<(), String> {
    db.add_scan_dir(&path);
    Ok(())
}

#[tauri::command]
fn remove_scan_dir(path: String, db: State<'_, Db>) -> Result<(), String> {
    db.remove_scan_dir(&path);
    Ok(())
}

#[tauri::command]
fn get_last_state(db: State<'_, Db>) -> Result<Option<LastState>, String> {
    Ok(db.get_last_state())
}

#[tauri::command]
fn get_songs(db: State<'_, Db>) -> Result<Vec<music_lib::Song>, String> {
    Ok(db.get_songs())
}

#[derive(serde::Serialize)]
struct LyricLine {
    time: f64,
    text: String,
}

#[tauri::command]
fn get_cover(path: String, app: AppHandle) -> Result<Option<String>, String> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let covers = data_dir.join("covers");
    std::fs::create_dir_all(&covers).map_err(|e| e.to_string())?;
    // 磁盘缓存：同一首歌只解析一次，且返回文件路径而非 base64，避免前端内存膨胀
    let key = fnv1a(&path);
    let cache = covers.join(format!("{key}.img"));
    if cache.is_file() {
        return Ok(cache.to_str().map(|s| s.to_string()));
    }
    let tagged = Probe::open(&path)
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    if let Some(tag) = tag {
        for pic in tag.pictures() {
            let data = pic.data();
            if data.len() > 256 && data.len() < 8_000_000 {
                std::fs::write(&cache, data).map_err(|e| e.to_string())?;
                return Ok(cache.to_str().map(|s| s.to_string()));
            }
        }
    }
    Ok(None)
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn parse_lrc_time(t: &str) -> Option<f64> {
    let (min, sec) = t.split_once(':')?;
    let m: f64 = min.trim().parse().ok()?;
    let s: f64 = sec.trim().parse().ok()?;
    Some(m * 60.0 + s)
}

fn decode_lrc_bytes(bytes: &[u8]) -> String {
    let body = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    if let Ok(s) = std::str::from_utf8(body) {
        return s.trim_start_matches('\u{feff}').to_string();
    }
    if let Ok(s) = String::from_utf16(
        &body
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<_>>(),
    ) {
        return s;
    }
    let (decoded, _, _) = encoding_rs::GBK.decode(body);
    decoded.into_owned()
}

#[tauri::command]
fn get_lyrics(path: String) -> Result<Vec<LyricLine>, String> {
    let p = std::path::Path::new(&path);
    let lrc = p.with_extension("lrc");
    eprintln!("[lyrics] req path={path} lrc_exists={}", lrc.is_file());
    if !lrc.is_file() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&lrc).map_err(|e| e.to_string())?;
    let content = decode_lrc_bytes(&bytes);
    let mut lines = Vec::new();
    for raw in content.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        loop {
            if let Some(stripped) = rest.strip_prefix('[') {
                if let Some(idx) = stripped.find(']') {
                    let t = &stripped[..idx];
                    if let Some(secs) = parse_lrc_time(t) {
                        times.push(secs);
                        rest = stripped[idx + 1..].trim_start();
                        continue;
                    }
                }
            }
            break;
        }
        let text = rest.trim();
        if times.is_empty() || text.is_empty() {
            continue;
        }
        for t in times {
            lines.push(LyricLine { time: t, text: text.to_string() });
        }
    }
    lines.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    eprintln!("[lyrics] parsed {} lines", lines.len());
    Ok(lines)
}

#[tauri::command]
fn play_stream(path: String, channel: Channel<StreamMsg>) -> Result<(), String> {
    player::play_stream(path, channel);
    Ok(())
}

#[tauri::command]
fn seek_stream(seconds: f64) -> Result<(), String> {
    player::seek_stream(seconds)
}

#[tauri::command]
fn ack_chunks(n: u64) {
    player::ack_chunks(n);
}

#[tauri::command]
fn set_paused(paused: bool) {
    player::set_paused(paused);
}
