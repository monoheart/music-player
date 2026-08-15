use lofty::file::AudioFile;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use serde::Serialize;
use std::borrow::Cow;
use std::path::Path;
use walkdir::WalkDir;

// 仅收录 symphonia 原生可解码的格式；opus/ape/wma/dsf 等不再支持（已移除 ffmpeg）
pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "mp2", "flac", "wav", "wave", "ogg", "oga", "m4a", "mp4", "aac", "aiff", "aif", "pcm",
];

#[derive(Debug, Clone, Serialize)]
pub struct Song {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub format: String,
    pub track: Option<u32>,
    pub year: Option<u32>,
    pub genre: String,
    #[serde(default)]
    pub fav: i32,
    #[serde(default)]
    pub sort_pos: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub format: String,
    pub size: u64,
    pub sample_rate: u32,
    pub bit_rate: u32,
    pub channels: u16,
    pub duration: f64,
}

/// 读取单曲文件信息（大小/采样率/比特率/声道/时长）
pub fn file_info(path: &str) -> Option<FileInfo> {
    let p = Path::new(path);
    let size = std::fs::metadata(p).ok()?.len();
    let tagged = Probe::open(p).ok()?.read().ok()?;
    let props = tagged.properties();
    let duration = props.duration().as_secs_f64();
    // lofty 的 bitrate 单位是 kbps；读不到时用整体比特率，仍为 0 则按 文件大小×8/时长 估算
    let bit_rate = props
        .audio_bitrate()
        .or_else(|| props.overall_bitrate())
        .unwrap_or_else(|| {
            if duration > 0.0 && size > 0 {
                (size as f64 * 8.0 / duration / 1000.0) as u32
            } else {
                0
            }
        });
    Some(FileInfo {
        path: path.to_string(),
        format: ext_of(p).unwrap_or_default(),
        size,
        sample_rate: props.sample_rate().unwrap_or(0),
        bit_rate,
        channels: props.channels().map(|c| c as u16).unwrap_or(0),
        duration,
    })
}

fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

pub fn is_audio_file(path: &Path) -> bool {
    ext_of(path).map_or(false, |e| AUDIO_EXTS.contains(&e.as_str()))
}

fn read_song(path: &Path) -> Option<Song> {
    let tagged = Probe::open(path).ok()?.read().ok()?;
    let props = tagged.properties();
    let duration = props.duration().as_secs_f64();

    let tag = tagged.tags().first();
    let mut song = Song {
        path: path.to_string_lossy().to_string(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        duration,
        format: ext_of(path).unwrap_or_default(),
        track: None,
        year: None,
        genre: String::new(),
        fav: 0,
        sort_pos: 0,
    };

    if let Some(t) = tag {
        song.title = t
            .title()
            .or_else(|| t.get_string(&ItemKey::TrackTitle).map(|s| Cow::<str>::Owned(s.to_owned())))
            .unwrap_or_default()
            .to_string();
        song.artist = t
            .artist()
            .or_else(|| t.get_string(&ItemKey::TrackArtist).map(|s| Cow::<str>::Owned(s.to_owned())))
            .unwrap_or_default()
            .to_string();
        song.album = t.album().unwrap_or_default().to_string();
        song.track = t.track();
        song.year = t.year();
        song.genre = t
            .get_string(&ItemKey::Genre)
            .unwrap_or_default()
            .to_string();
    }

    if song.title.is_empty() {
        song.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
    }
    if song.artist.is_empty() {
        song.artist = "未知艺术家".to_string();
    }
    if song.album.is_empty() {
        song.album = "未知专辑".to_string();
    }

    Some(song)
}

pub fn scan_dir(dir: &str) -> Vec<Song> {
    let mut songs = Vec::new();
    for entry in WalkDir::new(dir).follow_links(false).into_iter().flatten() {
        let p = entry.path();
        if !p.is_file() || !is_audio_file(p) {
            continue;
        }
        if let Some(s) = read_song(p) {
            songs.push(s);
        }
    }
    songs.sort_by(|a, b| {
        a.artist
            .cmp(&b.artist)
            .then_with(|| a.album.cmp(&b.album))
            .then_with(|| a.track.unwrap_or(0).cmp(&b.track.unwrap_or(0)))
            .then_with(|| a.title.cmp(&b.title))
    });
    songs
}
