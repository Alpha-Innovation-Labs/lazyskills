mod skills_tui_app;

use std::time::Duration;

use ratkit::prelude::{run as run_tui, RunnerConfig};

pub fn run() -> anyhow::Result<()> {
    let app = skills_tui_app::SkillsTuiApp::new(".agents")?;
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(200),
        ..RunnerConfig::default()
    };

    run_tui(app, config)?;
    Ok(())
}
