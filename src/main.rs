mod config;
mod embedding;
mod web;
pub use crate::web::web_entry;

use clap::{arg, Command};

fn cli() -> Command {
    Command::new("simgen")
        .about("A simple portable vector database for all your needs")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("serve")
                .about("Starts server")
                .arg(
                    arg!(--"port" <INTEGER>)
                        .short('p')
                        .value_parser(clap::value_parser!(u16)),
                )
                .arg(arg!(--"host" <STRING>))
                .arg(
                    arg!(--"config" <PATH>)
                        .short('c')
                        .default_value("config.toml"),
                ),
        )
}

fn main() {
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("serve", sub_m)) => {
            let config_file = sub_m
                .get_one::<String>("config")
                .map(String::as_str)
                .unwrap_or("config.toml");

            let config = config::Config::from_file(config_file);
            web_entry(config).unwrap();
        }
        _ => {
            println!("No subcommand found");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_serve_defaults() {
        let app = cli();
        let matches = app.try_get_matches_from(vec!["simgen", "serve"]).unwrap();

        if let Some(("serve", sub_m)) = matches.subcommand() {
            assert_eq!(sub_m.get_one::<String>("config").unwrap(), "config.toml");
            assert_eq!(sub_m.get_one::<String>("host"), None);
            assert_eq!(sub_m.get_one::<u16>("port"), None);
        }
    }

    #[test]
    fn test_cli_serve_with_args() {
        let app = cli();
        let matches = app
            .try_get_matches_from(vec![
                "simgen",
                "serve",
                "--port",
                "8080",
                "--host",
                "localhost",
                "--config",
                "test.toml",
            ])
            .unwrap();

        if let Some(("serve", sub_m)) = matches.subcommand() {
            assert_eq!(sub_m.get_one::<String>("config").unwrap(), "test.toml");
            assert_eq!(sub_m.get_one::<String>("host").unwrap(), "localhost");
            assert_eq!(sub_m.get_one::<u16>("port").unwrap(), &8080u16);
        }
    }

    #[test]
    #[should_panic]
    fn test_cli_invalid_port() {
        let app = cli();
        app.try_get_matches_from(vec!["simgen", "serve", "--port", "999999"])
            .unwrap();
    }
}
