use anyhow::{Context, Result};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use urlencoding::encode;

use crate::types::{AvDetail, AvItem, MagnetInfo};

const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36";

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    headers
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers(default_headers())
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        .build()
        .expect("client build")
}

pub async fn fetch_detail(code: &str) -> Result<AvDetail> {
    // Use javdb mirrored search as primary, fallback to sukebei
    let code_upper = code.to_uppercase();
    if let Ok(mut detail) = fetch_detail_from_javdb(&code_upper).await {
        if detail.magnets.is_empty() {
            if let Ok(s_detail) = fetch_detail_from_sukebei(&code_upper).await {
                if !s_detail.magnets.is_empty() {
                    detail.magnets = s_detail.magnets;
                }
            }
        }
        return Ok(detail);
    }
    fetch_detail_from_sukebei(&code_upper).await
}

pub async fn search(query: &str) -> Result<Vec<AvItem>> {
    let q = query.trim();
    if looks_like_code(q) {
        if let Ok(detail) = fetch_detail(q).await {
            return Ok(vec![AvItem { code: detail.code, title: detail.title }]);
        }
    }
    let mut items = search_javdb(q).await.unwrap_or_default();
    if items.is_empty() {
        items = search_sukebei(q).await.unwrap_or_default();
    }
    Ok(items)
}

pub async fn list_actor_titles(actor: &str) -> Result<Vec<AvItem>> {
    let mut items = list_actor_javdb(actor).await.unwrap_or_default();
    if items.is_empty() {
        items = list_actor_sukebei(actor).await.unwrap_or_default();
    }
    Ok(items)
}

fn looks_like_code(s: &str) -> bool {
    let re = Regex::new(r"(?i)^[a-z]{2,5}-?\d{2,5}").unwrap();
    re.is_match(s)
}

async fn fetch_detail_from_javdb(code: &str) -> Result<AvDetail> {
    let c = client();
    let url = format!("https://javdb.com/search?q={}&f=all", encode(code));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let card_sel = Selector::parse(".movie-list .item a.box.cover").unwrap();
    let first = doc.select(&card_sel).next().context("JavDB 未找到该番号")?;
    let href = first.value().attr("href").context("缺少链接")?;
    let detail_url = if href.starts_with("http") { href.to_string() } else { format!("https://javdb.com{}", href) };
    parse_javdb_detail(&c, &detail_url).await
}

async fn parse_javdb_detail(c: &reqwest::Client, url: &str) -> Result<AvDetail> {
    let body = c.get(url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let title_sel = Selector::parse(".title strong, h2.title").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|n| n.text().collect::<String>())
        .unwrap_or_else(|| {
            doc.select(&Selector::parse("title").unwrap())
                .next()
                .map(|n| n.text().collect::<String>())
                .unwrap_or_default()
        });

    let meta_sel = Selector::parse(".panel-block .value").unwrap();
    let mut code = String::new();
    let mut date: Option<String> = None;
    for val in doc.select(&meta_sel) {
        let txt = val.text().collect::<String>().trim().to_string();
        if code.is_empty() && looks_like_code(&txt) { code = txt.to_uppercase(); }
        if txt.contains('-') && txt.len() == 10 && txt.chars().nth(4) == Some('-') { date = Some(txt); }
    }

    let cover_sel = Selector::parse(".video-cover img").unwrap();
    let mut cover_url = doc
        .select(&cover_sel)
        .next()
        .and_then(|n| n.value().attr("src"))
        .map(|s| s.to_string());
    if cover_url.is_none() {
        let og_sel = Selector::parse("meta[property='og:image']").unwrap();
        cover_url = doc
            .select(&og_sel)
            .next()
            .and_then(|n| n.value().attr("content"))
            .map(|s| s.to_string());
    }

    let actor_sel = Selector::parse(".panel-block a[href*='/actors/'], a[href*='/actors/']").unwrap();
    let actor_names = doc
        .select(&actor_sel)
        .map(|n| n.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    // Init advanced fields before filling
    let mut duration_minutes: Option<u32> = None;
    let mut director: Option<String> = None;
    let mut studio: Option<String> = None;
    let mut label: Option<String> = None;
    let mut series: Option<String> = None;
    let mut genres: Vec<String> = Vec::new();
    let mut rating: Option<f32> = None;

    // Additional named links
    let get_one_text = |selector: &str| -> Option<String> {
        let s = Selector::parse(selector).ok()?;
        doc.select(&s)
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .filter(|t| !t.is_empty())
    };

    if let Some(v) = get_one_text("a[href*='/directors/']") { director = Some(v); }
    if let Some(v) = get_one_text("a[href*='/studios/']") { studio = Some(v); }
    if let Some(v) = get_one_text("a[href*='/labels/']") { label = Some(v); }
    if let Some(v) = get_one_text("a[href*='/series/']") { series = Some(v); }

    let plot_sel = Selector::parse(".panel-block .value pre, .panel-block .value p").unwrap();
    let plot = doc
        .select(&plot_sel)
        .map(|n| n.text().collect::<String>().trim().to_string())
        .find(|s| s.len() > 10);

    // Parse key/value meta rows (JavDB often uses dl/dt/dd or blocks). We'll look for dt labels.

    // Fallback: scan labeled anchors
    let label_link_sel = Selector::parse(".panel-block a.tag, .panel-block a[href*='/tags/']").unwrap();
    for a in doc.select(&label_link_sel) {
        let t = a.text().collect::<String>().trim().to_string();
        if !t.is_empty() {
            genres.push(t);
        }
    }
    genres.sort();
    genres.dedup();

    // Heuristics for duration and rating
    let body_text = doc.root_element().text().collect::<String>();
    if let Some(mins) = Regex::new(r"(\d{2,3})\s*min")
        .unwrap()
        .captures(&body_text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
    {
        duration_minutes = Some(mins);
    }
    if duration_minutes.is_none() {
        if let Some(mins2) = Regex::new(r"(\d{2,3})\s*(分钟|分|min|MIN)")
            .unwrap()
            .captures(&body_text)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
        {
            duration_minutes = Some(mins2);
        }
    }
    if let Some(r) = Regex::new(r"Rating\s*([0-9]+(?:\.[0-9]+)?)")
        .unwrap()
        .captures(&body_text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
    {
        rating = Some(r);
    }
    if rating.is_none() {
        if let Some(r2) = Regex::new(r"评分\s*([0-9]+(?:\.[0-9]+)?)|Score\s*([0-9]+(?:\.[0-9]+)?)")
            .unwrap()
            .captures(&body_text)
            .and_then(|c| c.get(1).or(c.get(2)))
            .and_then(|m| m.as_str().parse::<f32>().ok())
        {
            rating = Some(r2);
        }
    }

    // Release date robust regex
    if date.is_none() {
        if let Some(d) = Regex::new(r"(20\d{2}-\d{2}-\d{2})")
            .unwrap()
            .captures(&body_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
        {
            date = Some(d);
        }
    }

    // Try to parse some named fields by nearby labels
    let meta_row_sel = Selector::parse(".panel-block").unwrap();
    for row in doc.select(&meta_row_sel) {
        let label_text = row
            .select(&Selector::parse(".header, dt").unwrap())
            .next()
            .map(|n| n.text().collect::<String>())
            .unwrap_or_default();
        let value_text = row
            .select(&Selector::parse(".value, dd").unwrap())
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let lt = label_text.trim();
        if lt.contains("导演") || lt.contains("Director") {
            if !value_text.is_empty() { director = Some(value_text.clone()); }
        }
        if lt.contains("片商") || lt.contains("Studio") {
            if !value_text.is_empty() { studio = Some(value_text.clone()); }
        }
        if lt.contains("厂牌") || lt.contains("Label") {
            if !value_text.is_empty() { label = Some(value_text.clone()); }
        }
        if lt.contains("系列") || lt.contains("Series") {
            if !value_text.is_empty() { series = Some(value_text.clone()); }
        }
        if lt.contains("时长") || lt.contains("Length") {
            if let Some(m) = Regex::new(r"(\d{2,3})").unwrap().captures(&value_text).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<u32>().ok()) {
                duration_minutes = Some(m);
            }
        }
        if lt.contains("评分") || lt.contains("Rating") {
            if let Some(v) = Regex::new(r"([0-9]+(?:\.[0-9]+)?)").unwrap().captures(&value_text).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f32>().ok()) {
                rating = Some(v);
            }
        }
    }

    // Preview images
    let preview_sel = Selector::parse(".preview-images img, .samples .column img, .tile.is-child img, .sample-box img").unwrap();
    let preview_images = doc
        .select(&preview_sel)
        .filter_map(|img| img.value().attr("src"))
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let magnets = extract_magnets_from_text(&body);
    let magnet_infos = extract_magnet_infos_from_javdb(&doc, &magnets);
    Ok(AvDetail {
        code,
        title,
        actor_names,
        release_date: date,
        cover_url,
        plot,
        duration_minutes,
        director,
        studio,
        label,
        series,
        genres,
        rating,
        preview_images,
        magnet_infos,
        magnets,
    })
}

async fn fetch_detail_from_sukebei(code: &str) -> Result<AvDetail> {
    let c = client();
    let url = format!("https://sukebei.nyaa.si/?f=0&c=0_0&q={}", encode(code));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let row_sel = Selector::parse("table.torrent-list tbody tr").unwrap();
    let title_sel = Selector::parse("td[colspan] a, td:nth-child(2) a").unwrap();
    let mut first_link: Option<String> = None;
    let mut first_title: String = String::new();
    let mut first_row_html: Option<scraper::element_ref::ElementRef> = None;
    for row in doc.select(&row_sel) {
        if let Some(a) = row.select(&title_sel).next() {
            let t = a.text().collect::<String>();
            if t.to_uppercase().contains(code) {
                if let Some(href) = a.value().attr("href") {
                    first_link = Some(href.to_string());
                    first_title = t;
                    first_row_html = Some(row);
                    break;
                }
            }
        }
    }
    let page_url = first_link.context("Sukebei 未找到该番号")?;
    let detail_url = if page_url.starts_with("http") { page_url } else { format!("https://sukebei.nyaa.si{}", page_url) };
    let mut detail = parse_sukebei_detail(&c, &detail_url, code, &first_title).await?;

    // Try to enrich magnet_infos from the row
    if let Some(row) = first_row_html {
        let tds: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
        let magnet = row
            .select(&Selector::parse("a[href^='magnet:']").unwrap())
            .next()
            .and_then(|a| a.value().attr("href"))
            .map(|s| s.to_string());
        let size = tds.get(3).map(|n| n.text().collect::<String>().trim().to_string());
        let date = tds.get(4).map(|n| n.text().collect::<String>().trim().to_string());
        let seeders = tds
            .get(5)
            .and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());
        let leechers = tds
            .get(6)
            .and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());
        let downloads = tds
            .get(7)
            .and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());

        if let Some(mag) = magnet.clone() {
            let mi = MagnetInfo {
                url: mag.clone(),
                name: Some(first_title.clone()),
                size,
                date,
                seeders,
                leechers,
                downloads,
            };
            // insert if not exists
            let exists = detail.magnet_infos.iter().any(|x| x.url == mi.url);
            if !exists {
                detail.magnet_infos.push(mi);
            }
            // also ensure magnets list contains it
            if !detail.magnets.iter().any(|m| m == &mag) {
                detail.magnets.push(mag);
            }
        }
    }

    Ok(detail)
}

async fn parse_sukebei_detail(c: &reqwest::Client, url: &str, code: &str, title_guess: &str) -> Result<AvDetail> {
    let body = c.get(url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let title_sel = Selector::parse(".torrent-name").unwrap();
    let title_text = doc
        .select(&title_sel)
        .next()
        .map(|n| n.text().collect::<String>())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| title_guess.to_string());

    let magnet_sel = Selector::parse("a[href^='magnet:']").unwrap();
    let magnets = doc
        .select(&magnet_sel)
        .filter_map(|n| n.value().attr("href"))
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let magnet_infos = extract_magnet_infos_from_sukebei(&doc, &magnets);

    Ok(AvDetail {
        code: code.to_uppercase(),
        title: title_text,
        actor_names: vec![],
        release_date: None,
        cover_url: None,
        plot: None,
        duration_minutes: None,
        director: None,
        studio: None,
        label: None,
        series: None,
        genres: Vec::new(),
        rating: None,
        preview_images: Vec::new(),
        magnet_infos,
        magnets,
    })
}

async fn search_javdb(query: &str) -> Result<Vec<AvItem>> {
    let c = client();
    let url = format!("https://javdb.com/search?q={}&f=all", encode(query));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let card_sel = Selector::parse(".movie-list .item a.box.cover").unwrap();
    let title_sel = Selector::parse(".video-title").unwrap();
    let mut items = Vec::new();
    for a in doc.select(&card_sel) {
        let href = a.value().attr("href").unwrap_or("");
        let title = a
            .select(&title_sel)
            .next()
            .map(|n| n.text().collect::<String>())
            .unwrap_or_default();
        let code = extract_code_from_title(&title).unwrap_or_else(|| href.split('/').last().unwrap_or("").to_string());
        if !code.is_empty() && !title.is_empty() {
            items.push(AvItem { code: code.to_uppercase(), title });
        }
    }
    Ok(items)
}

async fn search_sukebei(query: &str) -> Result<Vec<AvItem>> {
    let c = client();
    let url = format!("https://sukebei.nyaa.si/?f=0&c=0_0&q={}", encode(query));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let row_sel = Selector::parse("table.torrent-list tbody tr").unwrap();
    let title_sel = Selector::parse("td[colspan] a, td:nth-child(2) a").unwrap();
    let mut items = Vec::new();
    for row in doc.select(&row_sel) {
        if let Some(a) = row.select(&title_sel).next() {
            let title = a.text().collect::<String>();
            if let Some(code) = extract_code_from_title(&title) {
                items.push(AvItem { code: code.to_uppercase(), title });
            }
        }
    }
    Ok(items)
}

async fn list_actor_javdb(actor: &str) -> Result<Vec<AvItem>> {
    let c = client();
    let url = format!("https://javdb.com/search?q={}&f=actor", encode(actor));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let card_sel = Selector::parse(".movie-list .item a.box.cover").unwrap();
    let title_sel = Selector::parse(".video-title").unwrap();
    let mut items = Vec::new();
    for a in doc.select(&card_sel) {
        let title = a
            .select(&title_sel)
            .next()
            .map(|n| n.text().collect::<String>())
            .unwrap_or_default();
        if let Some(code) = extract_code_from_title(&title) {
            items.push(AvItem { code: code.to_uppercase(), title });
        }
    }
    Ok(items)
}

async fn list_actor_sukebei(actor: &str) -> Result<Vec<AvItem>> {
    search_sukebei(actor).await
}

fn extract_code_from_title(title: &str) -> Option<String> {
    let re = Regex::new(r"(?i)([a-z]{2,5})[-_ ]?(\d{2,5})").unwrap();
    if let Some(caps) = re.captures(title) {
        let code = format!("{}-{}", &caps[1].to_uppercase(), &caps[2]);
        return Some(code);
    }
    None
}

fn extract_magnets_from_text(body: &str) -> Vec<String> {
    let re = Regex::new(r#"magnet:\?xt=urn:[^"'\s<>]+"#).unwrap();
    re.find_iter(body).map(|m| m.as_str().to_string()).collect()
}

fn extract_magnet_infos_from_javdb(doc: &Html, magnets: &Vec<String>) -> Vec<MagnetInfo> {
    // JavDB may not expose table data for magnets in HTML, so primarily return URLs
    magnets
        .iter()
        .map(|m| MagnetInfo { url: m.clone(), name: None, size: None, date: None, seeders: None, leechers: None, downloads: None })
        .collect()
}

fn extract_magnet_infos_from_sukebei(doc: &Html, magnets: &Vec<String>) -> Vec<MagnetInfo> {
    // sukebei detail page has a table with info, but mapping rows to magnets can be complex; best-effort
    let mut infos: Vec<MagnetInfo> = Vec::new();
    for m in magnets {
        infos.push(MagnetInfo { url: m.clone(), name: None, size: None, date: None, seeders: None, leechers: None, downloads: None });
    }
    infos
}


