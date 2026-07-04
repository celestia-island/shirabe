use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "shirabe", about = "Lightweight headless browser automation")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start standalone browser + debug API server.
    Debug {
        /// Port to listen on.
        #[arg(short, long, default_value = "3001")]
        port: u16,

        /// Initial URL.
        #[arg(short, long, default_value = "about:blank")]
        url: String,

        /// Proxy server for Chrome.
        #[arg(long)]
        proxy: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install ring crypto provider for rustls (required with rustls-no-provider feature)
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.cmd {
        Some(Command::Debug { port, url, proxy }) => {
            tracing::info!("Starting shirabe debug server on port {}", port);

            let cfg = shirabe::DebugServerConfig {
                base_url: url,
                dev_port: 0,
                dist_dir: "(standalone)".to_string(),
                package_name: "shirabe".to_string(),
                proxy,
            };

            shirabe::start_debug_server(cfg, port).await?;
        }
        None => {
            eprintln!("Usage: shirabe debug --port 3001 [--proxy http://localhost:7890]");
        }
    }

    Ok(())
}
