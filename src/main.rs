mod app;
mod cli;
mod config;
mod exitcode;
mod input;
mod model;
mod slack;
mod view;

use std::env;

fn main() {
    cli::parse_args();

    let config = config::load().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(exitcode::config_load_error());
    });

    let time_window = config.time_window_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid time_window: {}", e);
        std::process::exit(exitcode::invalid_time_window());
    });

    let poll_interval = config.poll_interval_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid poll_interval: {}", e);
        std::process::exit(exitcode::invalid_poll_interval());
    });

    let xoxd = env::var("SLACK9S_XOXD").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_XOXD environment variable not set");
        std::process::exit(exitcode::missing_xoxd());
    });

    let xoxc = env::var("SLACK9S_XOXC").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_XOXC environment variable not set");
        std::process::exit(exitcode::missing_xoxc());
    });

    let workspace_url = env::var("SLACK9S_WORKSPACE_URL").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_WORKSPACE_URL environment variable not set");
        std::process::exit(exitcode::missing_workspace_url());
    });
    let workspace_url_for_links = workspace_url.trim_end_matches('/').to_string();

    let mut client = slack::SlackClient::new(workspace_url, xoxd, xoxc);

    match client.auth_test() {
        Ok(response) if response.ok => {}
        Ok(response) => {
            eprintln!("Auth failed: {}", response.error.unwrap_or_else(|| "unknown error".to_string()));
            std::process::exit(exitcode::auth_rejected());
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(exitcode::request_failed());
        }
    }

    client.load_users().unwrap_or_else(|e| {
        eprintln!("Error loading users: {}", e);
        std::process::exit(exitcode::user_load_error());
    });

    let all_channels = client.list_channels().unwrap_or_else(|e| {
        eprintln!("Error listing channels: {}", e);
        std::process::exit(exitcode::channel_resolve_error());
    });

    let app = app::App::new(client, config, all_channels, workspace_url_for_links, time_window, poll_interval);
    app.run();
}
