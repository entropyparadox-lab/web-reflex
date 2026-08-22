pub mod server;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use web_reflex_core::SkeletonHasher;
use web_reflex_engine::{FastPathResult, ReplayEngine};
use web_reflex_storage::ActionStorage;

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
        /// SQLite database file path
        #[arg(short, long, default_value = "reflex.db")]
        db: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
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
        Commands::Serve { host, port, db } => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            let storage = Arc::new(ActionStorage::open(db)?);
            server::run_server(addr, storage).await?;
        }
    }

    Ok(())
}
