mod cli;
mod exitcode;
mod slack;

use std::env;

fn main() {
    cli::parse_args();

    let xoxd = env::var("SLACKEMON_XOXD").unwrap_or_else(|_| {
        eprintln!("Error: SLACKEMON_XOXD environment variable not set");
        std::process::exit(exitcode::missing_xoxd());
    });

    let xoxc = env::var("SLACKEMON_XOXC").unwrap_or_else(|_| {
        eprintln!("Error: SLACKEMON_XOXC environment variable not set");
        std::process::exit(exitcode::missing_xoxc());
    });

    let workspace_url = env::var("SLACKEMON_WORKSPACE_URL").unwrap_or_else(|_| {
        eprintln!("Error: SLACKEMON_WORKSPACE_URL environment variable not set");
        std::process::exit(exitcode::missing_workspace_url());
    });

    match slack::auth_test(&workspace_url, &xoxd, &xoxc) {
        Ok(response) if response.ok => {
            println!(
                "Authenticated as {} in {}",
                response.user.unwrap_or_default(),
                response.team.unwrap_or_default()
            );
        }
        Ok(response) => {
            eprintln!("Auth failed: {}", response.error.unwrap_or_else(|| "unknown error".to_string()));
            std::process::exit(exitcode::auth_rejected());
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(exitcode::request_failed());
        }
    }
}
