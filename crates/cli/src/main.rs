pub mod server;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use web_reflex_core::{ActionGraph, SkeletonHasher};
use web_reflex_engine::{FastPathResult, ReplayEngine};
use web_reflex_storage::{ActionStorage, TenantStoragePool};

#[derive(Parser)]
#[command(name = "web-reflex")]
#[command(about = "Deterministic, instant, and self-healing action engine for AI web agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute the skeleton hash of an HTML file
    Hash {
        /// Path to the HTML file
        file: PathBuf,
    },
    /// Inspect an HTML page against the local action cache
    Inspect {
        /// Path to the HTML file
        file: PathBuf,
        /// Optional SQLite cache path
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Start the WebReflex local HTTP/REST daemon
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to listen on
        #[arg(short, long, default_value_t = 9199)]
        port: u16,
        /// SQLite database file path (single-tenant mode)
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// SQLite databases directory (multi-tenant mode)
        #[arg(long)]
        db_dir: Option<PathBuf>,
        /// Optional API Key for bearer token authentication
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Migrate SQLite database schemas to the latest version
    Migrate {
        /// SQLite database file path (single-tenant mode)
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// SQLite databases directory (multi-tenant mode)
        #[arg(long)]
        db_dir: Option<PathBuf>,
    },
    /// View usage metrics, cache hit rate, and self-healing telemetry
    Stats {
        /// SQLite database file path (single-tenant mode)
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// SQLite databases directory (multi-tenant mode)
        #[arg(long)]
        db_dir: Option<PathBuf>,
    },
    /// Export all cached action graphs to JSON files for Git version control
    Export {
        /// Output directory
        #[arg(short, long, default_value = "recipes")]
        out: PathBuf,
        /// SQLite database file path
        #[arg(short, long, default_value = "reflex.db")]
        db: PathBuf,
    },
    /// Import action graph JSON files into SQLite database
    Import {
        /// Input directory containing JSON recipes
        #[arg(short, long, default_value = "recipes")]
        input: PathBuf,
        /// SQLite database file path
        #[arg(short, long, default_value = "reflex.db")]
        db: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Hash { file } => {
            let html = fs::read_to_string(&file)?;
            let hash = SkeletonHasher::compute_hash(&html);
            println!("Skeleton Hash: {}", hash);
        }
        Commands::Inspect { file, db } => {
            let html = fs::read_to_string(&file)?;
            let db_path = db.unwrap_or_else(|| PathBuf::from("reflex.db"));
            let storage = Arc::new(ActionStorage::open(db_path)?);
            let engine = ReplayEngine::new(storage);

            match engine.inspect_page(&html)? {
                FastPathResult::Hit(graph) => {
                    println!(
                        "🎯 Cache HIT: Graph '{}' (v{})",
                        graph.graph_id, graph.version
                    );
                    println!("Nodes: {}", graph.nodes.len());
                }
                FastPathResult::DomainCandidate {
                    graph,
                    current_skeleton_hash,
                } => {
                    println!(
                        "🔍 Domain Candidate: Graph '{}' (v{}), current skeleton hash: {}",
                        graph.graph_id, graph.version, current_skeleton_hash
                    );
                }
                FastPathResult::Miss { skeleton_hash } => {
                    println!("⚡ Cache MISS: Skeleton Hash {}", skeleton_hash);
                }
            }
        }
        Commands::Serve {
            host,
            port,
            db,
            db_dir,
            api_key,
        } => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            let pool = if let Some(dir) = db_dir {
                Arc::new(TenantStoragePool::new_directory(dir)?)
            } else {
                let db_path = db.unwrap_or_else(|| PathBuf::from("reflex.db"));
                let storage = Arc::new(ActionStorage::open(db_path)?);
                Arc::new(TenantStoragePool::new_single(storage))
            };
            let api_key = api_key.or_else(|| std::env::var("REFLEX_API_KEY").ok());
            server::run_server(addr, pool, api_key).await?;
        }
        Commands::Migrate { db, db_dir } => {
            let pool = if let Some(dir) = db_dir {
                TenantStoragePool::new_directory(dir)?
            } else {
                let db_path = db.unwrap_or_else(|| PathBuf::from("reflex.db"));
                let storage = Arc::new(ActionStorage::open(db_path)?);
                TenantStoragePool::new_single(storage)
            };
            println!(
                "🔄 Running SQLite schema migrations to v{}...",
                web_reflex_storage::CURRENT_SCHEMA_VERSION
            );
            let results = pool.migrate_all()?;
            for (tenant, version) in &results {
                println!("  ✅ Tenant '{}': schema v{}", tenant, version);
            }
            println!("🎉 Migrated {} database(s) successfully.", results.len());
        }
        Commands::Stats { db, db_dir } => {
            let pool = if let Some(dir) = db_dir {
                TenantStoragePool::new_directory(dir)?
            } else {
                let db_path = db.unwrap_or_else(|| PathBuf::from("reflex.db"));
                let storage = Arc::new(ActionStorage::open(db_path)?);
                TenantStoragePool::new_single(storage)
            };

            let all_stats = pool.get_all_stats()?;
            println!("============================================================");
            println!("📊 WebReflex Telemetry & Action Cache Performance Stats");
            println!("============================================================");

            for stats in all_stats {
                println!("\n🏢 Tenant: {}", stats.tenant_id);
                println!("  • Cached Action Recipes: {}", stats.total_graphs);
                println!("  • Cumulative Cache Hits:  {}", stats.total_hits);
                println!(
                    "  • Estimated Tokens Saved: ~{} tokens (${:.3} equivalent)",
                    stats.estimated_tokens_saved,
                    (stats.estimated_tokens_saved as f64 / 1_000_000.0) * 0.15
                );
                println!("  • Total Self-Heal Events: {}", stats.total_heals);

                if !stats.recent_heals.is_empty() {
                    println!("  • Recent Self-Healing History:");
                    for heal in &stats.recent_heals {
                        println!(
                            "    [{}] Graph '{}' (v{}) | Step: '{}' | {} -> {}",
                            heal.created_at,
                            heal.graph_id,
                            heal.version,
                            heal.step_id,
                            heal.old_selector,
                            heal.new_selector,
                        );
                    }
                }
            }
            println!("\n============================================================");
        }
        Commands::Export { out, db } => {
            let storage = ActionStorage::open(db)?;
            let graphs = storage.list_all_graphs()?;
            fs::create_dir_all(&out)?;

            let mut count = 0;
            for graph in graphs {
                let domain_dir = out.join(&graph.domain_pattern);
                fs::create_dir_all(&domain_dir)?;
                let file_path = domain_dir.join(format!("{}.json", graph.graph_id));
                let json_pretty = serde_json::to_string_pretty(&graph)?;
                fs::write(&file_path, json_pretty)?;
                count += 1;
            }
            println!("📦 Exported {} action recipes to {:?}", count, out);
        }
        Commands::Import { input, db } => {
            let storage = ActionStorage::open(db)?;
            let mut count = 0;

            if input.exists() && input.is_dir() {
                for entry in walk_dir(&input)? {
                    if entry.extension().and_then(|s| s.to_str()) == Some("json") {
                        let content = fs::read_to_string(&entry)?;
                        if let Ok(graph) = serde_json::from_str::<ActionGraph>(&content) {
                            storage.save_graph(&graph)?;
                            count += 1;
                        }
                    }
                }
            }
            println!("📥 Imported {} action recipes into database", count);
        }
    }

    Ok(())
}

fn walk_dir(dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_dir(&path)?);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}
