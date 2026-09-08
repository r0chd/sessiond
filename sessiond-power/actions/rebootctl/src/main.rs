use sessiond_power_client::Action;

fn main() -> anyhow::Result<()> {
    sessiond_power_client::run(Action::Reboot)
}
