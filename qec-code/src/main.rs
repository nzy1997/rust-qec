use clap::Parser;
use qec_code::cli::{Cli, run};
use qec_code::QecError;
use std::io::{self, Write};

fn main() {
    let cli = Cli::parse();
    let exit_code = run_and_write(cli, &mut io::stdout(), &mut io::stderr());
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run_and_write(cli: Cli, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    write_result(run(cli), stdout, stderr)
}

fn write_result(
    result: Result<String, QecError>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    match result {
        Ok(output) => write_success(stdout, &output),
        Err(error) => write_error(stderr, &error),
    }
}

fn write_success(stdout: &mut impl Write, output: &str) -> i32 {
    writeln!(stdout, "{output}").expect("stdout write should succeed");
    0
}

fn write_error(stderr: &mut impl Write, error: &qec_code::QecError) -> i32 {
    writeln!(stderr, "{error}").expect("stderr write should succeed");
    1
}

#[cfg(test)]
mod tests {
    use super::{run_and_write, write_result};
    use qec_code::cli::{Cli, CodeCommands, Commands, SteaneCommands};
    use qec_code::QecError;

    #[test]
    fn run_and_write_writes_stdout_on_success() {
        let cli = Cli {
            command: Commands::Code {
                command: CodeCommands::Steane {
                    command: SteaneCommands::Summary,
                },
            },
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_and_write(cli, &mut stdout, &mut stderr);

        assert_eq!(exit_code, 0);
        assert!(String::from_utf8(stdout).unwrap().contains("name: steane"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn write_result_writes_stderr_on_error() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = write_result(
            Err(QecError::DistanceWitnessNotFound),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(exit_code, 1);
        assert!(stdout.is_empty());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "distance witness not found\n"
        );
    }

    #[test]
    fn write_result_writes_stdout_on_success() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = write_result(Ok("ok".to_string()), &mut stdout, &mut stderr);

        assert_eq!(exit_code, 0);
        assert_eq!(String::from_utf8(stdout).unwrap(), "ok\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn qec_errors_render_human_readable_messages() {
        assert_eq!(
            QecError::DistanceWitnessNotFound.to_string(),
            "distance witness not found"
        );
    }
}
