mod cli;
mod exitcode;

fn main() {
    cli::parse_args();

    println!("Hello world!");

    std::process::exit(exitcode::example_error());
}
