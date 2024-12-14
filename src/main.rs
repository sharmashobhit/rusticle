mod web;

pub use crate::web::web_entry;

use clap::{arg, Command};

fn cli() -> Command {
    Command::new("simgen")
        .about("A simple portable vector database for all your needs")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("serve").about("Starts server"))
}
fn main() {
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("serve", _)) => {
            web_entry().unwrap();
        }
        _ => {
            println!("No subcommand found");
        }
    }
}
