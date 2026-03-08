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
    let debug = cli::parse_args();

    let config = config::load();

    let past = config.header.past_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid past: {}", e);
        std::process::exit(exitcode::invalid_past());
    });

    let poll = config.header.poll_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid poll: {}", e);
        std::process::exit(exitcode::invalid_poll());
    });

    let workspace_url = env::var("SLACK9_WORKSPACE").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9_WORKSPACE environment variable not set");
        std::process::exit(exitcode::missing_workspace());
    });

    let xoxd = env::var("SLACK9_XOXD").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9_XOXD environment variable not set");
        std::process::exit(exitcode::missing_xoxd());
    });

    let xoxc = env::var("SLACK9_XOXC").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9_XOXC environment variable not set");
        std::process::exit(exitcode::missing_xoxc());
    });

    let mut client = slack::SlackClient::new(workspace_url, xoxd, xoxc, debug);

    let (team_id, team_name, user_id) = match client.auth_test() {
        Ok(response) if response.ok => {
            let id = response.team_id.unwrap_or_else(|| {
                eprintln!("Error: auth.test did not return a team_id");
                std::process::exit(exitcode::missing_team_id());
            });
            let name = response.team.unwrap_or_else(|| id.clone());
            let uid = response.user_id.unwrap_or_else(|| {
                eprintln!("Error: auth.test did not return a user_id");
                std::process::exit(exitcode::missing_user_id());
            });
            (id, name, uid)
        }
        Ok(response) => {
            eprintln!("Auth failed: {}", response.error.unwrap_or_else(|| "unknown error".to_string()));
            std::process::exit(exitcode::auth_rejected());
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(exitcode::request_failed());
        }
    };

    client.load_users().unwrap_or_else(|e| {
        eprintln!("Error loading users: {}", e);
        std::process::exit(exitcode::user_load_error());
    });

    let all_channels = client.list_channels().unwrap_or_else(|e| {
        eprintln!("Error listing channels: {}", e);
        std::process::exit(exitcode::channel_resolve_error());
    });

    let app = app::App::new(client, config, all_channels, team_id, team_name, user_id, past, poll);
    app.run();
}
