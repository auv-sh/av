use anyhow::{Context, Result};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, HeaderName, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use urlencoding::encode;

use crate::types::{AvDetail, AvItem, MagnetInfo, ActorItem};
use std::collections::HashMap;
use crate::sources::{dmm, javlibrary};
use crate::util;

const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36";

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9,ja;q=0.8,zh-CN;q=0.7"));
    let referer = format!("{}/", javdb_base());
    if let Ok(hv) = HeaderValue::from_str(&referer) { headers.insert(REFERER, hv); }
    if let Some(cookie) = std::env::var("AV_JAVDB_COOKIE").ok() {
        let name = HeaderName::from_static("cookie");
        if let Ok(val) = HeaderValue::from_str(cookie.trim()) {
            headers.insert(name, val);
        }
    }
    headers
}

fn client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .default_headers(default_headers())
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        ;
    if let Some(proxy) = std::env::var("AV_HTTP_PROXY").ok() {
        if let Ok(px) = reqwest::Proxy::all(proxy) { builder = builder.proxy(px); }
    }
    builder.build().expect("client build")
}

fn javdb_base() -> String {
    std::env::var("AV_JAVDB_BASE").unwrap_or_else(|_| "https://javdb.com".to_string())
}

pub async fn fetch_detail(code: &str) -> Result<AvDetail> {
    // Prefer JavDB native scraping by default; DMM is opt-in via env AV_USE_DMM=1
    let code_upper = code.to_uppercase();
    util::debug(format!("fetch_detail start for {}", code_upper));
    if std::env::var("AV_USE_DMM").ok().as_deref() == Some("1") && dmm::dmm_enabled() {
        if let Some(mut d) = dmm::fetch_detail_from_dmm(&code_upper).await? {
            util::debug("DMM hit");
            // Merge with JavDB for plot/actors/cover fallback
            if let Ok(j) = fetch_detail_from_javdb(&code_upper).await {
                util::debug("Merging with JavDB after DMM");
                if d.plot.is_none() && j.plot.is_some() { d.plot = j.plot; }
                if d.actor_names.is_empty() && !j.actor_names.is_empty() { d.actor_names = j.actor_names; }
                if d.cover_url.is_none() && j.cover_url.is_some() { d.cover_url = j.cover_url; }
                // Prefer DMM release_date/duration if present; else copy from JavDB
                if d.release_date.is_none() { d.release_date = j.release_date; }
                if d.duration_minutes.is_none() { d.duration_minutes = j.duration_minutes; }
            }
            // Always merge magnets from Sukebei
            if let Ok(s) = fetch_detail_from_sukebei(&code_upper).await {
                if d.magnets.is_empty() { d.magnets = s.magnets; }
                if d.magnet_infos.is_empty() { d.magnet_infos = s.magnet_infos; }
            }
            return Ok(d);
        }
    }
    if let Ok(mut detail) = fetch_detail_from_javdb(&code_upper).await {
        util::debug("JavDB hit");
        // Merge extra metadata from JavLibrary even when JavDB succeeds
        if let Ok(Some(jl)) = javlibrary::fetch_detail_from_javlibrary(&code_upper).await {
            util::debug("Merging with JavLibrary after JavDB");
            if detail.plot.is_none() && jl.plot.is_some() { detail.plot = jl.plot; }
            if detail.actor_names.is_empty() && !jl.actor_names.is_empty() { detail.actor_names = jl.actor_names; }
            if detail.release_date.is_none() && jl.release_date.is_some() { detail.release_date = jl.release_date; }
            if detail.cover_url.is_none() && jl.cover_url.is_some() { detail.cover_url = jl.cover_url; }
            if detail.duration_minutes.is_none() && jl.duration_minutes.is_some() { detail.duration_minutes = jl.duration_minutes; }
            if detail.director.is_none() && jl.director.is_some() { detail.director = jl.director; }
            if detail.studio.is_none() && jl.studio.is_some() { detail.studio = jl.studio; }
            if detail.label.is_none() && jl.label.is_some() { detail.label = jl.label; }
            if detail.series.is_none() && jl.series.is_some() { detail.series = jl.series; }
            if detail.genres.is_empty() && !jl.genres.is_empty() { detail.genres = jl.genres; }
            if detail.preview_images.is_empty() && !jl.preview_images.is_empty() { detail.preview_images = jl.preview_images; }
        }
        if detail.magnets.is_empty() {
            if let Ok(s_detail) = fetch_detail_from_sukebei(&code_upper).await {
                if !s_detail.magnets.is_empty() {
                    detail.magnets = s_detail.magnets;
                }
            }
        }
        return Ok(detail);
    }
    // Try JavLibrary
    if let Ok(Some(mut jl)) = javlibrary::fetch_detail_from_javlibrary(&code_upper).await {
        util::debug("JavLibrary hit (fallback)");
        if let Ok(s) = fetch_detail_from_sukebei(&code_upper).await {
            if jl.magnets.is_empty() { jl.magnets = s.magnets; }
            if jl.magnet_infos.is_empty() { jl.magnet_infos = s.magnet_infos; }
        }
        return Ok(jl);
    }
    util::debug("Falling back to Sukebei only detail");
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

pub async fn top(limit: usize) -> Result<Vec<AvItem>> {
    // Try multiple ordering pages on JavDB: most recent, trending, etc.
    let c = client();
    let mut items: Vec<AvItem> = Vec::new();
    let endpoints = [
        format!("{}/videos?o=mr", javdb_base()), // most recent
        format!("{}/videos?o=tr", javdb_base()), // trending
    ];
    let card_sel = Selector::parse(".movie-list .item a.box.cover, .movie-list a[href^='/v/'], a.box[href^='/v/']").unwrap();
    let title_sel = Selector::parse(".video-title").unwrap();
    for url in &endpoints {
        util::debug(format!("JavDB top page: {}", url));
        let body = c.get(url).send().await?.error_for_status()?.text().await?;
        let doc = Html::parse_document(&body);
        for a in doc.select(&card_sel) {
            let href = a.value().attr("href").unwrap_or("");
            let title = a.select(&title_sel).next().map(|n| n.text().collect::<String>()).unwrap_or_else(|| a.text().collect::<String>());
            let code = extract_code_from_title(&title).unwrap_or_else(|| href.split('/').last().unwrap_or("").to_string());
            if !code.is_empty() && !title.is_empty() {
                items.push(AvItem { code: code.to_uppercase(), title });
                if items.len() >= limit { return Ok(items); }
            }
        }
    }
    Ok(items)
}

async fn fetch_detail_from_javdb(code: &str) -> Result<AvDetail> {
    let c = client();
    let url = format!("{}/search?q={}&f=all", javdb_base(), encode(code));
    util::debug(format!("JavDB search: {}", url));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    // If search redirected or rendered directly to detail page
    if doc.select(&Selector::parse(".video-meta-panel").unwrap()).next().is_some() {
        util::debug("JavDB: search rendered detail page directly");
        return parse_javdb_detail(&c, &url).await;
    }
    // Try several selectors to find the first result link
    let candidates = [
        ".movie-list .item a.box.cover",
        ".movie-list a[href^='/v/']",
        "a.box[href^='/v/']",
        "a[href^='/v/']",
    ];
    let mut href: Option<String> = None;
    for sel in candidates {
        let s = Selector::parse(sel).unwrap();
        if let Some(a) = doc.select(&s).next() {
            if let Some(h) = a.value().attr("href") {
                href = Some(h.to_string());
                util::debug(format!("JavDB: picked result via selector '{}' => {}", sel, h));
                break;
            }
        }
    }
    let href = href.context("JavDB 未找到该番号")?;
    let detail_url = if href.starts_with("http") { href.to_string() } else { format!("{}{}", javdb_base(), href) };
    util::debug(format!("JavDB detail: {}", detail_url));
    parse_javdb_detail(&c, &detail_url).await
}

pub async fn get_play_url(code: &str) -> Result<String> {
    let c = client();
    let url = format!("{}/search?q={}&f=all", javdb_base(), encode(code));
    util::debug(format!("JavDB search for play: {}", url));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    
    // If search redirected or rendered directly to detail page
    let play_sel = Selector::parse(".cover-container[href*='play'], a.cover-container[href*='play'], a[href*='play']").unwrap();
    if let Some(play) = doc.select(&play_sel).next().and_then(|a| a.value().attr("href")) {
        let play_url = if play.starts_with("http") { play.to_string() } else { format!("{}{}", javdb_base(), play) };
        util::debug(format!("JavDB play URL: {}", play_url));
        return Ok(play_url);
    }
    
    // Try to get detail page first, then look for play link
    let candidates = [
        ".movie-list .item a.box.cover",
        ".movie-list a[href^='/v/']",
        "a.box[href^='/v/']",
        "a[href^='/v/']",
    ];
    let mut href: Option<String> = None;
    for sel in candidates {
        let s = Selector::parse(sel).unwrap();
        if let Some(a) = doc.select(&s).next() {
            if let Some(h) = a.value().attr("href") {
                href = Some(h.to_string());
                break;
            }
        }
    }
    
    if let Some(href) = href {
        let detail_url = if href.starts_with("http") { href } else { format!("{}{}", javdb_base(), href) };
        let detail_body = c.get(&detail_url).send().await?.error_for_status()?.text().await?;
        let detail_doc = Html::parse_document(&detail_body);
        
        // Look for play button on detail page
        if let Some(play) = detail_doc.select(&play_sel).next().and_then(|a| a.value().attr("href")) {
            let play_url = if play.starts_with("http") { play.to_string() } else { format!("{}{}", javdb_base(), play) };
            util::debug(format!("JavDB play URL from detail: {}", play_url));
            return Ok(play_url);
        }
    }
    
    // Fallback: just return the search URL
    Ok(url)
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

    let cover_sel = Selector::parse("img.video-cover, .video-cover img").unwrap();
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
    let mut actor_names = doc
        .select(&actor_sel)
        .map(|n| n.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    // Init advanced fields before filling (must be declared before panel parsing loop)
    let mut duration_minutes: Option<u32> = None;
    let mut director: Option<String> = None;
    let mut studio: Option<String> = None;
    let mut label: Option<String> = None;
    let mut series: Option<String> = None;
    let mut genres: Vec<String> = Vec::new();
    let mut rating: Option<f32> = None;

    // Parse structured blocks in the movie info panel
    let block_sel = Selector::parse("nav.panel.movie-panel-info .panel-block").unwrap();
    let strong_sel = Selector::parse("strong").unwrap();
    let value_sel = Selector::parse(".value").unwrap();
    for bl in doc.select(&block_sel) {
        let label_text = bl
            .select(&strong_sel)
            .next()
            .map(|n| n.text().collect::<String>())
            .unwrap_or_default()
            .to_lowercase();
        let value_node = bl.select(&value_sel).next();
        let value_text = value_node
            .as_ref()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if label_text.contains("id") && code.is_empty() {
            let raw = value_text.replace('\n', " ");
            let raw = raw.trim();
            if looks_like_code(raw) { code = raw.to_uppercase(); }
        }
        if label_text.contains("released") {
            if !value_text.is_empty() { date = Some(value_text.clone()); }
        }
        if label_text.contains("duration") {
            if let Some(m) = Regex::new(r"(\d{2,3})").unwrap().captures(&value_text).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<u32>().ok()) {
                duration_minutes = Some(m);
            }
        }
        if label_text.contains("director") {
            if let Some(a) = value_node.as_ref().and_then(|n| n.select(&Selector::parse("a").unwrap()).next()) {
                let name = a.text().collect::<String>().trim().to_string();
                if !name.is_empty() { director = Some(name); }
            }
        }
        if label_text.contains("maker") {
            if let Some(a) = value_node.as_ref().and_then(|n| n.select(&Selector::parse("a").unwrap()).next()) {
                let name = a.text().collect::<String>().trim().to_string();
                if !name.is_empty() { studio = Some(name); }
            }
        }
        if label_text.contains("rating") {
            if let Some(v) = Regex::new(r"([0-9]+(?:\.[0-9]+)?)").unwrap().captures(&value_text).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f32>().ok()) {
                rating = Some(v);
            }
        }
        if label_text.contains("tags") {
            let tags = value_node
                .as_ref()
                .map(|n| n.select(&Selector::parse("a").unwrap()).map(|a| a.text().collect::<String>().trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>())
                .unwrap_or_default();
            if !tags.is_empty() { genres = tags; }
        }
        if label_text.contains("actor") {
            let names = value_node
                .as_ref()
                .map(|n| n.select(&Selector::parse("a").unwrap()).map(|a| a.text().collect::<String>().trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>())
                .unwrap_or_default();
            if !names.is_empty() { actor_names = names; }
        }
    }

    // Init advanced fields before filling (already declared above)

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
    let mut plot = doc
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
    let mut preview_images = doc
        .select(&preview_sel)
        .filter_map(|img| img.value().attr("src"))
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let magnets = extract_magnets_from_text(&body);
    let magnet_infos = extract_magnet_infos_from_javdb(&doc, &magnets);

    // Try JSON-LD for richer metadata
    let (ld_plot, ld_minutes, ld_actors, ld_images, ld_studio) = extract_ld_json_metadata(&doc);
    if plot.is_none() && ld_plot.is_some() { plot = ld_plot; }
    if duration_minutes.is_none() { duration_minutes = ld_minutes; }
    if actor_names.is_empty() && !ld_actors.is_empty() { actor_names = ld_actors; }
    if preview_images.is_empty() && !ld_images.is_empty() { preview_images = ld_images; }
    if studio.is_none() && ld_studio.is_some() { studio = ld_studio; }
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
                resolution: None,
                codec: None,
                avg_bitrate_mbps: None,
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
    let url = format!("{}/search?q={}&f=all", javdb_base(), encode(query));
    let body = c.get(&url).send().await?.error_for_status()?.text().await?;
    let doc = Html::parse_document(&body);
    let card_sel = Selector::parse(".movie-list .item a.box.cover, .movie-list a[href^='/v/'], a.box[href^='/v/']").unwrap();
    let title_sel = Selector::parse(".video-title").unwrap();
    let mut items = Vec::new();
    for a in doc.select(&card_sel) {
        let href = a.value().attr("href").unwrap_or("");
        let title = a.select(&title_sel).next().map(|n| n.text().collect::<String>()).unwrap_or_else(|| a.text().collect::<String>());
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
    let url = format!("{}/search?q={}&f=actor", javdb_base(), encode(actor));
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

pub async fn actors(page: usize, per_page: usize, uncensored_only: bool) -> Result<(Vec<ActorItem>, usize)> {
    // Prefer uncensored actors grid when requested
    let c = client();
    let endpoints = if uncensored_only {
        vec![format!("{}/actors/uncensored?page={}", javdb_base(), page)]
    } else {
        vec![
            format!("{}/actors?o=tr&page={}", javdb_base(), page),
            format!("{}/rankings/actors?period=w&page={}", javdb_base(), page),
            format!("{}/rankings/actors?period=m&page={}", javdb_base(), page),
        ]
    };
    let mut all: Vec<ActorItem> = Vec::new();
    let mut total_pages: Option<usize> = None;

    for url in &endpoints {
        util::debug(format!("JavDB actors page: {}", url));
        let resp = c.get(url).send().await?;
        if !resp.status().is_success() { continue; }
        let body = resp.text().await?;
        let doc = Html::parse_document(&body);

        // Estimate total pages
        if total_pages.is_none() {
            let pages = doc
                .select(&Selector::parse(".pagination-list a.pagination-link").unwrap())
                .filter_map(|n| n.text().collect::<String>().trim().parse::<usize>().ok())
                .max();
            if let Some(p) = pages { total_pages = Some(p); }
        }

        // Prefer the actors grid structure: #actors .actor-box a strong
        let grid_sel = Selector::parse("#actors .actor-box a, .actors .actor-box a").unwrap();
        let strong_sel = Selector::parse("strong").unwrap();
        let mut grid: Vec<ActorItem> = Vec::new();
        for (idx, a) in doc.select(&grid_sel).enumerate() {
            let name_strong = a.select(&strong_sel).next().map(|n| n.text().collect::<String>().trim().to_string());
            let title_attr = a.value().attr("title").map(|s| s.to_string());
            // Some title has multiple names separated by comma; pick first
            let name_from_title = title_attr.clone().and_then(|t| t.split(',').next().map(|s| s.trim().to_string()));
            let name = name_strong.filter(|s| !s.is_empty()).or(name_from_title).unwrap_or_default();
            if name.is_empty() { continue; }
            // If no explicit hot metric, use order (descending)
            let hot_rank = (per_page as i64 - idx as i64).max(1) as u32;
            grid.push(ActorItem { name, hot: hot_rank });
        }
        if !grid.is_empty() {
            // apply per_page limit locally
            let mut limited = grid;
            if limited.len() > per_page { limited.truncate(per_page); }
            all = limited;
            break;
        }

        // Fallback: anchors-based heuristic (older layout)
        let a_sel = Selector::parse("a[href^='/actors/']").unwrap();
        let mut seen: HashMap<String, u32> = HashMap::new();
        for (idx, a) in doc.select(&a_sel).enumerate() {
            let name = a.text().collect::<String>().trim().to_string();
            if name.is_empty() { continue; }
            let hot_rank = (per_page as i64 - idx as i64).max(1) as u32;
            let entry = seen.entry(name).or_insert(0);
            if hot_rank > *entry { *entry = hot_rank; }
        }
        if !seen.is_empty() {
            let mut v = seen.into_iter().map(|(name, hot)| ActorItem { name, hot }).collect::<Vec<_>>();
            v.sort_by(|a, b| b.hot.cmp(&a.hot).then_with(|| a.name.cmp(&b.name)));
            all = v;
            break;
        }
    }

    // Fallback: return empty with total estimation if none found
    let total_pages = total_pages.unwrap_or(page);
    // If we have items count for this page, approximate total items
    let total_items = total_pages * per_page;
    Ok((all, total_items))
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

fn extract_ld_json_metadata(doc: &Html) -> (Option<String>, Option<u32>, Vec<String>, Vec<String>, Option<String>) {
    let script_sel = Selector::parse("script[type='application/ld+json']").unwrap();
    for sc in doc.select(&script_sel) {
        let text = sc.text().collect::<String>();
        if text.trim().is_empty() { continue; }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            // Look for VideoObject/Movie schemas
            let ctx = v.get("@type").and_then(|t| t.as_str()).unwrap_or("");
            if ctx.eq_ignore_ascii_case("VideoObject") || ctx.eq_ignore_ascii_case("Movie") {
                let plot = v.get("description").and_then(|x| x.as_str()).map(|s| s.trim().to_string());
                let duration_minutes = v.get("duration").and_then(|x| x.as_str()).and_then(parse_iso8601_duration_minutes);
                let actors = v.get("actor").and_then(|x| x.as_array()).map(|arr| {
                    arr.iter().filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())).collect::<Vec<_>>()
                }).unwrap_or_default();
                let images = v.get("image").map(|img| {
                    if let Some(s) = img.as_str() { vec![s.to_string()] } else if let Some(arr) = img.as_array() { arr.iter().filter_map(|i| i.as_str().map(|s| s.to_string())).collect() } else { vec![] }
                }).unwrap_or_default();
                let studio = v.get("productionCompany").and_then(|x| x.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string());
                return (plot, duration_minutes, actors, images, studio);
            }
        }
    }
    (None, None, Vec::new(), Vec::new(), None)
}

fn parse_iso8601_duration_minutes(s: &str) -> Option<u32> {
    // PT1H40M or PT100M
    let re = Regex::new(r"^PT(?:(\d+)H)?(?:(\d+)M)?$").ok()?;
    let caps = re.captures(s)?;
    let h = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
    let m = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
    Some(h * 60 + m)
}
fn extract_magnet_infos_from_javdb(_doc: &Html, magnets: &Vec<String>) -> Vec<MagnetInfo> {
    // JavDB may not expose table data for magnets in HTML, so primarily return URLs
    magnets
        .iter()
        .map(|m| MagnetInfo { url: m.clone(), name: None, size: None, date: None, seeders: None, leechers: None, downloads: None, resolution: None, codec: None, avg_bitrate_mbps: None })
        .collect()
}

fn extract_magnet_infos_from_sukebei(doc: &Html, magnets: &Vec<String>) -> Vec<MagnetInfo> {
    // sukebei detail page has a table with info, but mapping rows to magnets can be complex; best-effort
    let mut infos: Vec<MagnetInfo> = Vec::new();
    // Try to read title to infer resolution/codec/bitrate hints
    let title = doc
        .select(&Selector::parse(".torrent-name").unwrap())
        .next()
        .map(|n| n.text().collect::<String>())
        .unwrap_or_default();
    let res = Regex::new(r"(\d{3,4}p|\d{3,4}x\d{3,4})").ok()
        .and_then(|re| re.captures(&title)).map(|c| c.get(1).unwrap().as_str().to_string());
    let codec = Regex::new(r"(H\.264|H\.265|AVC|HEVC|x264|x265)").ok()
        .and_then(|re| re.captures(&title)).map(|c| c.get(1).unwrap().as_str().to_string());
    let mut size_text: Option<String> = None;
    let mut seeders: Option<u32> = None;
    let mut leechers: Option<u32> = None;
    let mut downloads: Option<u32> = None;
    // Table columns often: Category | Name | Link | Size | Date | S | L | C
    if let Some(row) = doc.select(&Selector::parse("table.torrent-list tbody tr").unwrap()).next() {
        let tds: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
        size_text = tds.get(3).map(|n| n.text().collect::<String>().trim().to_string());
        seeders = tds.get(5).and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());
        leechers = tds.get(6).and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());
        downloads = tds.get(7).and_then(|n| n.text().collect::<String>().trim().parse::<u32>().ok());
    }

    // Try to infer bitrate from size and rough duration if present on the page
    let mut avg_bitrate_mbps: Option<f32> = None;
    if let Some(size_s) = size_text.clone() {
        if let Some((bytes, _unit)) = parse_size_to_bytes(&size_s) {
            let body_text = doc.root_element().text().collect::<String>();
            if let Some(dur_min) = Regex::new(r"(\d{2,3})\s*(min|分钟)").ok()
                .and_then(|re| re.captures(&body_text))
                .and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<u32>().ok())
            {
                let bits = (bytes as f64) * 8.0;
                let sec = (dur_min as f64) * 60.0;
                let mbps = bits / sec / 1_000_000.0;
                avg_bitrate_mbps = Some(mbps as f32);
            }
        }
    }

    for m in magnets {
        infos.push(MagnetInfo {
            url: m.clone(),
            name: Some(title.clone()).filter(|s| !s.is_empty()),
            size: size_text.clone(),
            date: None,
            seeders,
            leechers,
            downloads,
            resolution: res.clone(),
            codec: codec.clone(),
            avg_bitrate_mbps,
        });
    }
    infos
}

fn parse_size_to_bytes(s: &str) -> Option<(u64, String)> {
    let re = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*([KMGT]i?B)").ok()?;
    let caps = re.captures(s)?;
    let num: f64 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str().to_uppercase();
    let mult = match unit.as_str() {
        "KB" | "KIB" => 1024.0,
        "MB" | "MIB" => 1024.0_f64.powi(2),
        "GB" | "GIB" => 1024.0_f64.powi(3),
        "TB" | "TIB" => 1024.0_f64.powi(4),
        _ => return None,
    };
    Some(((num * mult) as u64, unit))
}


