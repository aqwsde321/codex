use clap::Parser;
use codex_auth_proxy::Args;

#[ctor::ctor]
fn pre_main() {
    codex_process_hardening::pre_main_hardening();
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    codex_auth_proxy::run_main(args)
}
