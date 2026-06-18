use mentci::client::ClientCommand;

fn main() -> std::process::ExitCode {
    match ClientCommand::from_environment().run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mentci: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
