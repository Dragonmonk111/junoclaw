use junoclaw_core::config::JunoClawConfig;
use junoclaw_runtime::Runtime;

pub struct AppState {
    pub config: JunoClawConfig,
    pub runtime: Runtime,
}

impl AppState {
    pub fn new(config: JunoClawConfig, runtime: Runtime) -> Self {
        Self { config, runtime }
    }
}
