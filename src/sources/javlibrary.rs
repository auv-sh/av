use anyhow::Result;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};

use crate::types::AvDetail;
use crate::util;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125 Safari/537.36";

fn client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    reqwest::Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .expect("client build")
}

pub async fn fetch_detail_from_javlibrary(code: &str) -> Result<Option<AvDetail>> {
    let c = client();
    // Try multiple locales for better hit rate
    let locales = ["en", "cn", "ja"];
    let mut body = String::new();
    let mut found = false;
    for loc in &locales {
        let url = format!("https://www.javlibrary.com/{}/vl_searchbyid.php?keyword={}", loc, code);
        util::debug(format!("JavLibrary search: {}", url));
        let resp = c.get(&url).send().await?;
        if resp.status().is_success() {
            body = resp.text().await?;
            found = true;
            break;
        }
    }
    if !found { return Ok(None); }
    let doc = Html::parse_document(&body);
    let first_link = doc
        .select(&Selector::parse(".video a[href*='?v=']").unwrap())
        .next()
        .and_then(|a| a.value().attr("href"))
        .map(|s| s.to_string());
    let href = match first_link { Some(h) => h, None => return Ok(None) };
    let detail_url = if href.starts_with("http") { href } else { format!("https://www.javlibrary.com/en/{}", href.trim_start_matches('/')) };
    util::debug(format!("JavLibrary detail: {}", detail_url));

    let body = c.get(&detail_url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);

    let title = doc
        .select(&Selector::parse("#video_title").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let code_text = doc
        .select(&Selector::parse("#video_id .text").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| code.to_uppercase());

    let date = doc
        .select(&Selector::parse("#video_date .text").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string());

    let cover_url = doc
        .select(&Selector::parse("#video_jacket_img").unwrap())
        .next()
        .and_then(|n| n.value().attr("src"))
        .map(|s| s.to_string());

    let actor_names = doc
        .select(&Selector::parse("#video_cast .star a").unwrap())
        .map(|n| n.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let studio = doc
        .select(&Selector::parse("#video_maker .text a").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string());

    let label = doc
        .select(&Selector::parse("#video_label .text a").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string());

    let series = doc
        .select(&Selector::parse("#video_series .text a").unwrap())
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string());

    let mut duration_minutes = None;
    if let Some(t) = doc
        .select(&Selector::parse("#video_length .text").unwrap())
        .next()
        .map(|n| n.text().collect::<String>())
    {
        if let Some(cap) = Regex::new(r"(\d{2,3})").unwrap().captures(&t) {
            duration_minutes = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        }
    }

    // Genres
    let genres = doc
        .select(&Selector::parse("#video_genres .genre a").unwrap())
        .map(|n| n.text().collect::<String>().trim().to_string())
        .collect::<Vec<_>>();

    Ok(Some(AvDetail {
        code: code_text,
        title,
        actor_names,
        release_date: date,
        cover_url,
        plot: None,
        duration_minutes,
        director: None,
        studio,
        label,
        series,
        genres,
        rating: None,
        preview_images: Vec::new(),
        magnet_infos: Vec::new(),
        magnets: Vec::new(),
    }))
}


