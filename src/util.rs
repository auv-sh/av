use anyhow::{bail, Context, Result};
use colored::*;
use serde::Serialize;
use std::process::Stdio;
use which::which;
use std::env;
use std::fs;
use std::io::Write;
use std::fs::File;

// Unix-specific imports
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::types::AvItem;
use crate::types::AvDetail;
use crate::types::ActorItem;

use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn set_debug(on: bool) {
    DEBUG.store(on, Ordering::Relaxed);
}

pub fn is_debug() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

pub fn debug<S: AsRef<str>>(msg: S) {
    if is_debug() {
        eprintln!("[DEBUG] {}", msg.as_ref());
    }
}

pub fn print_output<T: Serialize + std::fmt::Debug>(value: &T, json: bool) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("{:?}", value),
        }
    } else {
        println!("{:?}", value);
    }
}

pub async fn download_via_aria2(magnet: &str) -> Result<()> {
    if which("aria2c").is_err() {
        bail!("未检测到 aria2c，请先安装: brew install aria2");
    }

    let mut cmd = tokio::process::Command::new("aria2c");
    cmd.arg("--seed-time=0").arg(magnet).stdin(Stdio::null());

    let status = cmd.status().await.context("启动 aria2c 失败")?;
    if !status.success() {
        bail!("aria2c 下载失败，退出码: {:?}", status.code());
    }
    println!("{} {}", "下载完成".green().bold(), magnet);
    Ok(())
}

pub async fn open_system_uri(uri: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = tokio::process::Command::new("open");
        c.arg(uri);
        c
    };

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = tokio::process::Command::new("xdg-open");
        c.arg(uri);
        c
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg("start").arg("").arg(uri);
        c
    };

    let status = cmd.status().await.context("调用系统打开 URI 失败")?;
    if !status.success() {
        bail!("系统无法打开: {}", uri);
    }
    println!("{} {}", "已交给系统默认的 BT 客户端处理".green().bold(), uri);
    Ok(())
}

pub async fn download_magnet(magnet: &str) -> Result<()> {
    if which("aria2c").is_ok() {
        download_via_aria2(magnet).await
    } else {
        open_system_uri(magnet).await
    }
}

pub async fn open_browser_url(url: &str) -> Result<()> {
    open_system_uri(url).await
}

pub async fn self_update() -> Result<()> {
    println!("正在检查更新...");
    
    // 创建临时目录
    let tmpdir = tempfile::tempdir().context("无法创建临时目录")?;
    let installer_path = tmpdir.path().join("install.sh");
    
    // 下载安装脚本
    let install_script_url = "https://raw.github.com/auv-sh/av/master/install.sh";
    println!("下载安装脚本: {}", install_script_url);
    
    let response = reqwest::get(install_script_url).await.context("下载安装脚本失败")?;
    let script_content = response.text().await.context("读取安装脚本内容失败")?;
    
    // 写入安装脚本到临时文件
    let mut file = File::create(&installer_path).context("创建临时安装脚本文件失败")?;
    file.write_all(script_content.as_bytes()).context("写入安装脚本内容失败")?;
    
    // 设置执行权限
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&installer_path).context("获取文件权限失败")?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&installer_path, perms).context("设置执行权限失败")?;
    }
    
    // Windows doesn't need explicit permission setting for execution
    
    // 获取当前可执行文件路径
    let _current_exe = env::current_exe().context("无法确定当前可执行文件路径")?;
    
    // 执行安装脚本
    println!("执行安装脚本...");
    
    #[cfg(unix)]
    let status = tokio::process::Command::new("sh")
        .arg(&installer_path)
        .status()
        .await
        .context("执行安装脚本失败")?;
        
    #[cfg(windows)]
    let status = {
        // Windows 需要使用 PowerShell 或 cmd 来执行脚本
        // 首先将 .sh 脚本内容转换为 .ps1 脚本
        let ps_path = tmpdir.path().join("install.ps1");
        let ps_content = script_content.replace("\r\n", "\n").replace("\n", "\r\n");
        let mut ps_file = File::create(&ps_path).context("创建 PowerShell 脚本文件失败")?;
        ps_file.write_all(ps_content.as_bytes()).context("写入 PowerShell 脚本内容失败")?;
        
        // 使用 PowerShell 执行脚本
        tokio::process::Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&ps_path)
            .status()
            .await
            .context("执行安装脚本失败")?
    };
    
    if status.success() {
        println!("{}", "更新成功！".green().bold());
    } else {
        bail!("安装脚本执行失败，退出码: {:?}", status.code());
    }
    
    Ok(())
}

pub fn print_items_table(items: &[AvItem]) {
    println!("{} {}", "共".bold(), items.len());

    let index_header = "#";
    let code_header = "番号";
    let title_header = "标题";

    let index_width = std::cmp::max(index_header.len(), format!("{}", items.len()).len());
    let code_width = std::cmp::max(
        code_header.len(),
        items.iter().map(|i| i.code.len()).max().unwrap_or(0),
    );

    println!(
        "{:<iw$}  {:<cw$}  {}",
        index_header.bold(),
        code_header.bold(),
        title_header.bold(),
        iw = index_width,
        cw = code_width
    );

    let sep_i = "-".repeat(index_width);
    let sep_c = "-".repeat(code_width);
    println!("{:<iw$}  {:<cw$}  {}", sep_i, sep_c, "-".repeat(10), iw = index_width, cw = code_width);

    for (idx, item) in items.iter().enumerate() {
        let row_index = idx + 1;
        println!(
            "{:<iw$}  {:<cw$}  {}",
            row_index,
            item.code,
            item.title,
            iw = index_width,
            cw = code_width
        );
    }
}

pub fn looks_uncensored(text: &str) -> bool {
    let lower = text.to_lowercase();
    let keywords = [
        "uncensored",
        "uncensored leak",
        "uncensored crack",
        "無修正",
        "无码",
        "無碼",
        "無修正流出",
        "無碼流出",
        "无码流出",
        "无修正",
    ];
    keywords.iter().any(|k| lower.contains(&k.to_lowercase()))
}

pub fn print_detail_human(detail: &AvDetail) {
    println!("番号： {}", detail.code.bold());
    println!("标题： {}", detail.title);
    if !detail.actor_names.is_empty() {
        println!("演员： {}", detail.actor_names.join(", "));
    }
    if let Some(date) = &detail.release_date {
        println!("发行： {}", date);
    }
    if let Some(cover) = &detail.cover_url {
        println!("封面： {}", cover);
    }
    if let Some(mins) = detail.duration_minutes {
        println!("时长： {} 分钟", mins);
    }
    if let Some(dir) = &detail.director {
        println!("导演： {}", dir);
    }
    if let Some(studio) = &detail.studio {
        println!("片商： {}", studio);
    }
    if let Some(label) = &detail.label {
        println!("厂牌： {}", label);
    }
    if let Some(series) = &detail.series {
        println!("系列： {}", series);
    }
    if !detail.genres.is_empty() {
        println!("类别： {}", detail.genres.join(", "));
    }
    if let Some(r) = detail.rating {
        println!("评分： {}", r);
    }
    if let Some(plot) = &detail.plot {
        println!("剧情：\n{}", plot);
    }
    if !detail.preview_images.is_empty() {
        println!("预览图：");
        for (i, url) in detail.preview_images.iter().enumerate() {
            println!("  {}. {}", i + 1, url);
        }
    }
    if !detail.magnets.is_empty() {
        println!("磁力： 共{}条", detail.magnets.len());
        for (i, m) in detail.magnet_infos.iter().take(5).enumerate() {
            let mut line = format!("  {}. {}", i + 1, m.url);
            if let Some(name) = &m.name { line.push_str(&format!("\n     {}", name)); }
            if let Some(size) = &m.size { line.push_str(&format!(" | {}", size)); }
            if let Some(res) = &m.resolution { line.push_str(&format!(" | {}", res)); }
            if let Some(codec) = &m.codec { line.push_str(&format!(" | {}", codec)); }
            if let Some(b) = m.avg_bitrate_mbps { line.push_str(&format!(" | ~{:.2} Mbps", b)); }
            if let Some(s) = m.seeders { line.push_str(&format!(" | S:{}", s)); }
            if let Some(lc) = m.leechers { line.push_str(&format!(" L:{}", lc)); }
            println!("{}", line);
        }
    }
}

pub fn print_actors_table(actors: &[ActorItem], page: usize, per_page: usize, total: usize) {
    println!("{} {} (page {} / {}):", "Total".bold(), total, page, ((total + per_page - 1) / per_page));
    let index_header = "#";
    let name_header = "演员";
    let hot_header = "热度";
    let index_width = std::cmp::max(index_header.len(), format!("{}", actors.len()).len());
    let name_width = std::cmp::max(name_header.len(), actors.iter().map(|a| a.name.len()).max().unwrap_or(0));
    println!(
        "{:<iw$}  {:<nw$}  {}",
        index_header.bold(),
        name_header.bold(),
        hot_header.bold(),
        iw = index_width,
        nw = name_width
    );
    println!(
        "{:<iw$}  {:<nw$}  {}",
        "-".repeat(index_width),
        "-".repeat(name_width),
        "-".repeat(6),
        iw = index_width,
        nw = name_width
    );
    for (i, a) in actors.iter().enumerate() {
        println!("{:<iw$}  {:<nw$}  {}", i + 1 + (page - 1) * per_page, a.name, a.hot, iw = index_width, nw = name_width);
    }
}

