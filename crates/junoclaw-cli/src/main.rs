use anyhow::Result;
use tracing::info;

use junoclaw_core::config::{
    JunoClawConfig, OllamaConfig,
};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "init" => cmd_init()?,
        "start" => cmd_start()?,
        "version" => cmd_version(),
        "help" | "--help" | "-h" => cmd_help(),
        other => {
            eprintln!("Unknown command: {}", other);
            cmd_help();
        }
    }

    Ok(())
}

fn cmd_init() -> Result<()> {
    let data_dir = JunoClawConfig::data_dir();
    let config_path = JunoClawConfig::default_path();

    if config_path.exists() {
        info!("Config already exists at {}", config_path.display());
        return Ok(());
    }

    // Create directory structure
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(JunoClawConfig::workspaces_dir().join("default"))?;
    std::fs::create_dir_all(JunoClawConfig::agents_dir())?;
    std::fs::create_dir_all(JunoClawConfig::sessions_dir())?;

    // Create default config with Ollama
    let mut config = JunoClawConfig::default();
    config.llm.providers.ollama = Some(OllamaConfig::default());

    config.save(&config_path)?;

    println!("✓ JunoClaw initialized at {}", data_dir.display());
    println!("  Config: {}", config_path.display());
    println!("  Workspaces: {}", JunoClawConfig::workspaces_dir().display());
    println!();
    println!("Next steps:");
    println!("  1. Ensure Ollama is running: ollama serve");
    println!("  2. Start the daemon: junoclaw start");
    println!("  3. Open http://localhost:7777");

    Ok(())
}

fn cmd_start() -> Result<()> {
    let config_path = JunoClawConfig::default_path();

    if !config_path.exists() {
        eprintln!("No config found. Run `junoclaw init` first.");
        std::process::exit(1);
    }

    println!("Starting JunoClaw daemon...");
    println!("Run the daemon directly: cargo run -p junoclaw-daemon");
    println!("(Binary distribution will embed the daemon in the CLI)");

    Ok(())
}

fn cmd_version() {
    println!("junoclaw {}", env!("CARGO_PKG_VERSION"));
}

fn cmd_help() {
    println!("JunoClaw — Open-source agentic AI platform on Juno Network");
    println!();
    println!("USAGE:");
    println!("  junoclaw <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("  init       Initialize JunoClaw (~/.junoclaw/)");
    println!("  start      Start the JunoClaw daemon");
    println!("  version    Print version");
    println!("  help       Show this help");
}
