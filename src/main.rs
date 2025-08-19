use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod scraper;
mod types;
mod util;
mod sources;

#[derive(Parser, Debug)]
#[command(name = "av", version, about = "AV CLI: 搜索、查看与下载番号和演员作品", long_about = None)]
struct Cli {
    /// 统一输出为 JSON
    #[arg(long, global = true)]
    json: bool,

    /// 输出调试日志
    #[arg(long, global = true)]
    debug: bool,

    /// 只显示无马赛克（基于标题/标签的启发式判断）
    #[arg(long = "uncen", short = 'u', alias = "nomo", global = true)]
    uncen: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 下载该番号对应的视频（磁力）
    #[command(alias = "get")]
    Install { code: String },

    /// 展示该番号的详细信息
    Detail { code: String },

    /// 列出该演员的所有番号
    List { actor: String },

    /// 搜索演员或番号
    Search { query: String },

    /// 查看最新的番（默认 20 条）
    Top { #[arg(short, long, default_value_t = 20)] limit: usize },

    /// 演员热度排行榜（分页）
    Actors { #[arg(short, long, default_value_t = 1)] page: usize, #[arg(short='n', long, default_value_t = 50)] per_page: usize },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    util::set_debug(cli.debug);

    match cli.command {
        Commands::Install { code } => {
            let detail = scraper::fetch_detail(&code).await?;
            let magnet = detail
                .magnet_infos
                .iter()
                .max_by_key(|m| m.seeders.unwrap_or(0))
                .map(|m| m.url.clone())
                .or_else(|| detail.magnets.get(0).cloned())
                .context("未找到可用的磁力链接")?;
            util::download_magnet(&magnet).await?;
            Ok(())
        }
        Commands::Detail { code } => {
            util::debug(format!("detail: fetching {}", code));
            let detail = scraper::fetch_detail(&code).await?;
            if cli.json {
                util::print_output(&detail, true);
            } else {
                util::print_detail_human(&detail);
            }
            Ok(())
        }
        Commands::List { actor } => {
            let mut items = scraper::list_actor_titles(&actor).await?;
            if cli.uncen {
                items.retain(|i| util::looks_uncensored(&i.title));
            }
            if cli.json {
                util::print_output(&items, true);
            } else {
                util::print_items_table(&items);
            }
            Ok(())
        }
        Commands::Search { query } => {
            let mut items = scraper::search(&query).await?;
            if cli.uncen {
                items.retain(|i| util::looks_uncensored(&i.title));
            }
            if cli.json {
                util::print_output(&items, true);
            } else {
                util::print_items_table(&items);
            }
            Ok(())
        }
        Commands::Top { limit } => {
            let mut items = scraper::top(limit).await?;
            if cli.uncen {
                items.retain(|i| util::looks_uncensored(&i.title));
            }
            if cli.json {
                util::print_output(&items, true);
            } else {
                util::print_items_table(&items);
            }
            Ok(())
        }
        Commands::Actors { page, per_page } => {
            let (actors, total) = scraper::actors(page, per_page, cli.uncen).await?;
            if cli.json {
                util::print_output(&(actors, total), true);
            } else {
                util::print_actors_table(&actors, page, per_page, total);
            }
            Ok(())
        }
    }
}
