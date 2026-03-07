use clap::command;

mod style;
use style::get_style;

pub fn parse_args() {
    command!().styles(get_style()).get_matches();
}
