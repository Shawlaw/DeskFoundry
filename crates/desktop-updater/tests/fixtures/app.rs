fn main() {
    if std::env::var("DESKTOP_UPDATER_INTEGRATION_MODE").as_deref() == Ok("ack") {
        if let Err(error) = desktop_updater::acknowledge_if_requested() {
            eprintln!("desktop-updater integration app: {error}");
            std::process::exit(1);
        }
    }
}
