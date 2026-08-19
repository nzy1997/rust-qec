use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    match rustqec_cli::run(std::env::args_os(), &mut stdin, &mut stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            rustqec_cli::write_error(&error, &mut stdout, &mut io::stderr().lock());
            ExitCode::from(rustqec_cli::exit_code(&error))
        }
    }
}
