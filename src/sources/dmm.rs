use anyhow::{Context, Result};
use reqwest::Url;
use serde_json::Value;

use crate::types::AvDetail;

fn env_api_id() -> Option<String> {
    std::env::var("DMM_API_ID").ok().filter(|s| !s.is_empty())
}

fn env_affiliate_id() -> Option<String> {
    std::env::var("DMM_AFFILIATE_ID").ok().filter(|s| !s.is_empty())
}

pub fn dmm_enabled() -> bool {
    env_api_id().is_some() && env_affiliate_id().is_some()
}

pub async fn fetch_detail_from_dmm(code: &str) -> Result<Option<AvDetail>> {
    if !dmm_enabled() {
        return Ok(None);
    }

    let api_id = env_api_id().unwrap();
    let affiliate_id = env_affiliate_id().unwrap();

    // Build ItemList API URL
    // See DMM Web Service docs; we search by keyword = code
    let mut url = Url::parse("https://api.dmm.com/affiliate/v3/ItemList").unwrap();
    url.query_pairs_mut()
        .append_pair("api_id", &api_id)
        .append_pair("affiliate_id", &affiliate_id)
        .append_pair("site", "DMM")
        .append_pair("service", "digital")
        .append_pair("floor", "videoa")
        .append_pair("hits", "1")
        .append_pair("sort", "-date")
        .append_pair("keyword", code);

    let resp_text = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .context("DMM request failed")?
        .error_for_status()
        .context("DMM non-success status")?
        .text()
        .await
        .context("DMM read body failed")?;

    let v: Value = serde_json::from_str(&resp_text).context("DMM parse json failed")?;
    let items = v
        .get("result")
        .and_then(|r| r.get("items"))
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return Ok(None);
    }
    let it = &items[0];

    // Helper closures for safe extraction
    let pick_string = |obj: &Value, path: &[&str]| -> Option<String> {
        let mut cur = obj;
        for p in path {
            cur = cur.get(*p)?;
        }
        cur.as_str().map(|s| s.to_string())
    };

    let title = pick_string(it, &["title"]).unwrap_or_default();
    // Image
    let cover_url = pick_string(it, &["imageURL", "large"]).or_else(|| pick_string(it, &["imageURL", "list"]));
    // Release date / Duration
    let release_date = pick_string(it, &["date"]).or_else(|| pick_string(it, &["volume"]));
    let duration_minutes = pick_string(it, &["review", "duration"]) // some mirrors
        .or_else(|| pick_string(it, &["duration"]))
        .and_then(|s| s.parse::<u32>().ok());

    // Actors, Genres, Studio/Label/Series
    let mut actor_names: Vec<String> = Vec::new();
    if let Some(acts) = it.get("iteminfo").and_then(|x| x.get("actress")).and_then(|x| x.as_array()) {
        for a in acts {
            if let Some(n) = pick_string(a, &["name"]) {
                actor_names.push(n);
            }
        }
    }
    let mut genres: Vec<String> = Vec::new();
    if let Some(gs) = it.get("iteminfo").and_then(|x| x.get("genre")).and_then(|x| x.as_array()) {
        for g in gs {
            if let Some(n) = pick_string(g, &["name"]) {
                genres.push(n);
            }
        }
    }
    let director = it.get("iteminfo").and_then(|x| x.get("director")).and_then(|x| x.as_array()).and_then(|arr| arr.get(0)).and_then(|d| pick_string(d, &["name"]));
    let studio = it.get("iteminfo").and_then(|x| x.get("maker")).and_then(|x| x.as_array()).and_then(|arr| arr.get(0)).and_then(|d| pick_string(d, &["name"]));
    let label = it.get("iteminfo").and_then(|x| x.get("label")).and_then(|x| x.as_array()).and_then(|arr| arr.get(0)).and_then(|d| pick_string(d, &["name"]));
    let series = it.get("iteminfo").and_then(|x| x.get("series")).and_then(|x| x.as_array()).and_then(|arr| arr.get(0)).and_then(|d| pick_string(d, &["name"]));

    // Rating (average)
    let rating = pick_string(it, &["review", "average"]).and_then(|s| s.parse::<f32>().ok());

    // Preview images (sample)
    let mut preview_images: Vec<String> = Vec::new();
    if let Some(samples) = it.get("sampleImageURL").and_then(|x| x.get("sample_s")).and_then(|x| x.get("image")).and_then(|x| x.as_array()) {
        for img in samples {
            if let Some(u) = img.as_str() {
                preview_images.push(u.to_string());
            }
        }
    }

    // Code: DMM may not echo vendor code. Fall back to the provided code.
    let code_upper = code.to_uppercase();

    let detail = AvDetail {
        code: code_upper,
        title,
        actor_names,
        release_date,
        cover_url,
        plot: None,
        duration_minutes,
        director,
        studio,
        label,
        series,
        genres,
        rating,
        preview_images,
        magnet_infos: Vec::new(),
        magnets: Vec::new(),
    };

    Ok(Some(detail))
}


