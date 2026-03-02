mod adapters;
mod app;

fn main() -> anyhow::Result<()> {
    app::run()
}
