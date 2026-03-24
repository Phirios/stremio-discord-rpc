use chrono::{DateTime, Utc};
use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{thread, time::Duration};

const DISCORD_APP_ID: &str = "1485708779478585445";
const POLL_INTERVAL: Duration = Duration::from_secs(10);
const ACTIVE_THRESHOLD_SECS: i64 = 60;
const STREMIO_ICON: &str = "https://www.stremio.com/website/stremio-logo-small.png";
// _mtime bu kadar saniyedir güncellenmemişse paused say
const PAUSE_THRESHOLD_SECS: i64 = 20;

fn main() {
    println!("Stremio Discord RPC başlatılıyor...");
    println!("Stremio ve Discord'un açık olduğundan emin ol.\n");

    let db_path = find_db_path();
    println!("DB yolu: {}\n", db_path.display());

    let mut client =
        DiscordIpcClient::new(DISCORD_APP_ID).expect("Discord IPC client oluşturulamadı");

    let mut connected = false;
    let mut was_playing = false;
    let mut last_video_id = String::new();
    let mut cached_episode: Option<EpisodeInfo> = None;
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP client oluşturulamadı");

    loop {
        if !connected {
            match client.connect() {
                Ok(_) => {
                    println!("Discord'a bağlanıldı!");
                    connected = true;
                }
                Err(_) => {
                    eprintln!("Discord'a bağlanılamadı, tekrar denenecek...");
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }
            }
        }

        match get_currently_watching(&db_path) {
            Some(item) => {
                let current_vid = item.video_id.clone().unwrap_or_default();
                let is_new = current_vid != last_video_id;

                if is_new {
                    cached_episode = fetch_episode_info(&http, &item);
                    last_video_id = current_vid.clone();

                    if let Some(ref ep) = cached_episode {
                        println!(
                            "İzleniyor: {} — {} {}",
                            item.name, ep.episode_label, ep.title
                        );
                    } else if item.item_type == "series" {
                        cached_episode = parse_episode_from_video_id(&current_vid);
                        println!(
                            "İzleniyor: {} — {}",
                            item.name,
                            cached_episode
                                .as_ref()
                                .map(|e| e.episode_label.as_str())
                                .unwrap_or("?")
                        );
                    } else {
                        println!("İzleniyor: {}", item.name);
                    }
                }

                // Poster URL — köşeleri yuvarlatılmış (wsrv.nl proxy)
                let raw_poster = item.poster.clone().unwrap_or_else(|| {
                    format!(
                        "https://images.metahub.space/poster/medium/{}/img",
                        item.id
                    )
                });
                let poster_url = round_image_url(&raw_poster);

                // Pause tespiti: _mtime yakın zamanda güncellenmediyse paused
                let is_paused = item.mtime_age_secs > PAUSE_THRESHOLD_SECS;

                // Details ve state
                let (details_text, state_text) = if item.item_type == "series" {
                    if let Some(ref ep) = cached_episode {
                        let title = if !ep.title.is_empty() {
                            format!("{} - {}", item.name, ep.title)
                        } else {
                            item.name.clone()
                        };
                        (title, format!("Sezon {} - Bölüm {}", ep.season, ep.episode))
                    } else {
                        (item.name.clone(), "İzleniyor".to_string())
                    }
                } else {
                    let status = if is_paused { "Paused" } else { "Playing" };
                    (item.name.clone(), status.to_string())
                };

                // Zaman hesapla
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;

                let elapsed_secs = (item.time_offset / 1000) as i64;
                let total_secs = (item.duration / 1000) as i64;
                let start_time = now_secs - elapsed_secs;

                // Progress bar — sadece oynatılıyorsa aktif, paused ise dondur
                let timestamps = if is_paused {
                    // Paused: sabit zaman göster (progress bar donmuş görünür)
                    if total_secs > 0 {
                        Timestamps::new()
                            .start(now_secs - elapsed_secs)
                            .end(now_secs - elapsed_secs + total_secs)
                    } else {
                        Timestamps::new().start(now_secs - elapsed_secs)
                    }
                } else {
                    // Playing: canlı ilerleme
                    let mut ts = Timestamps::new().start(start_time);
                    if total_secs > 0 {
                        ts = ts.end(start_time + total_secs);
                    }
                    ts
                };

                let activity = Activity::new()
                    .activity_type(discord_rich_presence::activity::ActivityType::Watching)
                    .details(&details_text)
                    .state(&state_text)
                    .assets(
                        Assets::new()
                            .large_image(&poster_url)
                            .large_text(&item.name)
                            .small_image(STREMIO_ICON)
                            .small_text("Stremio"),
                    )
                    .timestamps(timestamps);

                if client.set_activity(activity).is_err() {
                    eprintln!("Activity gönderilemedi, yeniden bağlanılıyor...");
                    connected = false;
                    let _ = client.close();
                    client = DiscordIpcClient::new(DISCORD_APP_ID)
                        .expect("Discord IPC client oluşturulamadı");
                }

                was_playing = true;
            }
            None => {
                if was_playing {
                    println!("Oynatma durdu, presence temizleniyor.");
                    let _ = client.clear_activity();
                    was_playing = false;
                    last_video_id.clear();
                    cached_episode = None;
                }
            }
        }

        thread::sleep(POLL_INTERVAL);
    }
}

struct EpisodeInfo {
    title: String,
    season: u32,
    episode: u32,
    episode_label: String,
}

#[derive(Debug)]
struct WatchingItem {
    id: String,
    name: String,
    item_type: String,
    poster: Option<String>,
    time_offset: u64,
    duration: u64,
    video_id: Option<String>,
    mtime_age_secs: i64,
}

#[derive(Deserialize)]
struct LibraryRecent {
    items: HashMap<String, LibraryItem>,
}

#[derive(Deserialize)]
struct LibraryItem {
    _id: String,
    name: String,
    #[serde(rename = "type")]
    item_type: String,
    poster: Option<String>,
    _mtime: String,
    #[serde(default)]
    state: Option<ItemState>,
}

#[derive(Deserialize)]
struct ItemState {
    #[serde(rename = "timeOffset")]
    time_offset: Option<u64>,
    duration: Option<u64>,
    video_id: Option<String>,
}

/// Poster URL'sini img.phirios.com üzerinden köşeleri yuvarlatılmış olarak döndür
fn round_image_url(url: &str) -> String {
    let encoded = urlencoding::encode(url);
    format!(
        "https://img.phirios.com/rounded?url={}&r=30&s=300",
        encoded
    )
}

fn find_db_path() -> PathBuf {
    let home = dirs::home_dir().expect("Home dizini bulunamadı");
    let webkit_base = home.join("Library/WebKit/com.westbridge.stremio5-mac/WebsiteData/Default");

    if let Ok(entries) = std::fs::read_dir(&webkit_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && entry.file_name() != "salt" {
                let ls_path = path
                    .join(entry.file_name())
                    .join("LocalStorage/localstorage.sqlite3");
                if ls_path.exists() {
                    return ls_path;
                }
            }
        }
    }

    panic!("Stremio localStorage veritabanı bulunamadı!");
}

fn get_currently_watching(db_path: &PathBuf) -> Option<WatchingItem> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

    let blob: Vec<u8> = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key='library_recent'",
            [],
            |row| row.get(0),
        )
        .ok()?;

    let text = utf16le_to_string(&blob)?;
    let library: LibraryRecent = serde_json::from_str(&text).ok()?;

    let now = Utc::now();
    let mut best: Option<(&LibraryItem, i64)> = None;

    for item in library.items.values() {
        if let Ok(mtime) = item._mtime.parse::<DateTime<Utc>>() {
            let age = now.signed_duration_since(mtime).num_seconds();
            if age <= ACTIVE_THRESHOLD_SECS {
                match &best {
                    Some((_, best_age)) if age < *best_age => {
                        best = Some((item, age));
                    }
                    None => {
                        best = Some((item, age));
                    }
                    _ => {}
                }
            }
        }
    }

    let (item, age) = best?;
    let state = item.state.as_ref()?;

    Some(WatchingItem {
        id: item._id.clone(),
        name: item.name.clone(),
        item_type: item.item_type.clone(),
        poster: item.poster.clone(),
        time_offset: state.time_offset.unwrap_or(0),
        duration: state.duration.unwrap_or(0),
        video_id: state.video_id.clone(),
        mtime_age_secs: age,
    })
}

fn utf16le_to_string(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }

    let u16_vec: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let start = if u16_vec.first() == Some(&0xFEFF) {
        1
    } else {
        0
    };

    String::from_utf16(&u16_vec[start..]).ok()
}

/// Cinemeta, Kitsu veya Animecix API'den bölüm bilgilerini çek
fn fetch_episode_info(
    http: &reqwest::blocking::Client,
    item: &WatchingItem,
) -> Option<EpisodeInfo> {
    let video_id = item.video_id.as_ref()?;

    if item.item_type != "series" {
        return None;
    }

    let (api_url, target_id) = if video_id.starts_with("kitsu:") {
        let parts: Vec<&str> = video_id.split(':').collect();
        if parts.len() < 2 {
            return None;
        }
        let url = format!(
            "https://anime-kitsu.strem.fun/meta/series/kitsu:{}.json",
            parts[1]
        );
        (url, video_id.as_str())
    } else if video_id.starts_with("tt") && video_id.contains(':') {
        let imdb_id = video_id.split(':').next()?;
        let url = format!(
            "https://v3-cinemeta.strem.io/meta/series/{}.json",
            imdb_id
        );
        (url, video_id.as_str())
    } else {
        let url = format!(
            "https://animecixnet-stremio-addon.mycodelab.com.tr/addon/meta/series/{}.json",
            item.id
        );
        (url, video_id.as_str())
    };

    let resp = http.get(&api_url).send().ok()?;
    let json: Value = resp.json().ok()?;
    let videos = json.get("meta")?.get("videos")?.as_array()?;

    for video in videos {
        let vid = video.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if vid == target_id {
            let title = video
                .get("name")
                .or_else(|| video.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            let season = video
                .get("season")
                .and_then(|s| s.as_u64())
                .unwrap_or(1) as u32;

            let episode = video
                .get("episode")
                .or_else(|| video.get("number"))
                .and_then(|e| e.as_u64())
                .unwrap_or(0) as u32;

            return Some(EpisodeInfo {
                title,
                season,
                episode,
                episode_label: format!("S{:02}E{:02}", season, episode),
            });
        }
    }

    None
}

/// video_id'den sezon/bölüm bilgisini parse et (API fallback)
fn parse_episode_from_video_id(video_id: &str) -> Option<EpisodeInfo> {
    let parts: Vec<&str> = video_id.split(':').collect();

    if parts.len() == 3 && parts[0].starts_with("tt") {
        if let (Ok(s), Ok(e)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
            return Some(EpisodeInfo {
                title: String::new(),
                season: s,
                episode: e,
                episode_label: format!("S{:02}E{:02}", s, e),
            });
        }
    }

    if parts.len() == 3 && parts[0] == "kitsu" {
        if let Ok(e) = parts[2].parse::<u32>() {
            return Some(EpisodeInfo {
                title: String::new(),
                season: 1,
                episode: e,
                episode_label: format!("Bölüm {}", e),
            });
        }
    }

    None
}
