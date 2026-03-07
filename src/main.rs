mod cli;
mod config;
mod exitcode;
mod slack;

use std::collections::HashSet;
use std::env;
use std::thread;

fn main() {
    cli::parse_args();

    let config = config::load().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(exitcode::config_load_error());
    });
    println!("{}", config);

    let time_window = config.time_window_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid time_window: {}", e);
        std::process::exit(exitcode::invalid_time_window());
    });

    let poll_interval = config.poll_interval_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid poll_interval: {}", e);
        std::process::exit(exitcode::invalid_poll_interval());
    });

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

    let mut client = slack::SlackClient::new(workspace_url, xoxd, xoxc);

    match client.auth_test() {
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

    println!(
        "\nPolling every {} for messages within {} window...\n",
        config.poll_interval, config.time_window
    );

    client.load_users().unwrap_or_else(|e| {
        eprintln!("Error loading users: {}", e);
        std::process::exit(exitcode::user_load_error());
    });

    let channels = client.resolve_channels(&config.channels).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(exitcode::channel_resolve_error());
    });

    for (id, name) in &channels {
        println!("  #{} ({})", name, id);
    }
    println!();

    let mut seen: HashSet<String> = HashSet::new();

    loop {
        for (channel_id, channel_name) in &channels {
            match client.conversations_history(channel_id, time_window) {
                Ok(resp) if resp.ok => {
                    if let Some(messages) = resp.messages {
                        for msg in messages.iter().rev() {
                            if seen.insert(msg.ts.clone()) {
                                let user = msg.user.as_deref().unwrap_or("unknown");
                                let display_name = client.resolve_user(user);
                                let text = msg.text.as_deref().unwrap_or("");
                                println!("[{}] #{} @{}: {}", msg.timestamp(), channel_name, display_name, text);
                            }
                        }
                    }
                }
                Ok(resp) => {
                    eprintln!(
                        "Error fetching #{}: {}",
                        channel_name,
                        resp.error.unwrap_or_else(|| "unknown error".to_string())
                    );
                }
                Err(e) => {
                    eprintln!("Error fetching #{}: {}", channel_name, e);
                }
            }
        }

        thread::sleep(poll_interval);
    }
}
