use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvDetail {
    pub code: String,
    pub title: String,
    pub actor_names: Vec<String>,
    pub release_date: Option<String>,
    pub cover_url: Option<String>,
    pub plot: Option<String>,
    pub duration_minutes: Option<u32>,
    pub director: Option<String>,
    pub studio: Option<String>,
    pub label: Option<String>,
    pub series: Option<String>,
    pub genres: Vec<String>,
    pub rating: Option<f32>,
    pub preview_images: Vec<String>,
    pub magnet_infos: Vec<MagnetInfo>,
    pub magnets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvItem {
    pub code: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnetInfo {
    pub url: String,
    pub name: Option<String>,
    pub size: Option<String>,
    pub date: Option<String>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub downloads: Option<u32>,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub avg_bitrate_mbps: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorItem {
    pub name: String,
    pub hot: u32,
}

