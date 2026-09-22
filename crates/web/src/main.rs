use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(name = "conn-web", version, about = "Conn local web host")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 1423)]
        port: u16,
        #[arg(long)]
        ui_dir: Option<PathBuf>,
        #[arg(long)]
        dev_origin: Option<String>,
        #[arg(long)]
        setup_home: Option<PathBuf>,
        #[arg(long)]
        connection_file: Option<PathBuf>,
    },
    Contract,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Contract => println!(
            "{}",
            serde_json::to_string_pretty(&conn_frontend::owner::contract())?
        ),
        Command::Serve {
            state_dir,
            host,
            port,
            ui_dir,
            dev_origin,
            setup_home,
            connection_file,
        } => {
            let state_dir = state_dir.unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("conn-web")
            });
            let ui_dir = ui_dir.or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.join("ui")))
            });
            let host = conn_web::WebHost::bind(conn_web::Config {
                state_dir,
                host,
                port,
                ui_dir,
                dev_origin,
                setup_home,
                connection_file,
            })
            .await?;
            println!("Conn web: {}", host.origin());
            println!(
                "Owner bootstrap: use the private connection file {}",
                host.connection_file().display()
            );
            host.serve().await?;
        }
    }
    Ok(())
}
