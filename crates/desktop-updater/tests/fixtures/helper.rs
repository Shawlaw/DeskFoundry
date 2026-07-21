fn main() {
    if let Err(error) = desktop_updater::run_helper_from_args() {
        eprintln!("desktop-updater integration helper: {error}");
        std::process::exit(1);
    }
}
