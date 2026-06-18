use mentci::command::DaemonCommand;

fn main() -> std::process::ExitCode {
    match DaemonCommand::from_environment().run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mentci-daemon: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
