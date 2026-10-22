use clap::Parser;

#[derive(Parser)]
#[command(version, about = "MoleAPI standalone API workbench server")]
pub(crate) struct Args {
    #[arg(long, env = "MOLEAPI_BIND", default_value = "127.0.0.1:8787", num_args = 0..=1, default_missing_value = "127.0.0.1:8787", value_parser = parse_bind)]
    pub(crate) bind: std::net::SocketAddr,
    /// Database connection URL; defaults to the SQLite database path.
    #[arg(long, env = "MOLEAPI_DATABASE_URL", num_args = 0..=1)]
    pub(crate) database_url: Option<String>,
    #[arg(long, env = "MOLEAPI_DATABASE", default_value = "./data/moleapi.db", num_args = 0..=1, default_missing_value = "./data/moleapi.db", value_parser = parse_database)]
    pub(crate) database: std::path::PathBuf,
    /// First-user setup token; generated randomly when omitted or empty.
    #[arg(long, env = "MOLEAPI_SETUP_TOKEN", num_args = 0..=1)]
    pub(crate) setup_token: Option<String>,
    /// Allow requests to private networks (enabled by default).
    #[arg(long, env = "MOLEAPI_ALLOW_PRIVATE_NETWORK", default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", value_parser = parse_bool::<true>)]
    pub(crate) allow_private_network: bool,
    /// Allow registration after the first account (disabled by default).
    #[arg(long, env = "MOLEAPI_ALLOW_REGISTRATION", default_value_t = false, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", value_parser = parse_bool::<false>)]
    pub(crate) allow_registration: bool,
}

fn parse_bind(value: &str) -> Result<std::net::SocketAddr, std::net::AddrParseError> {
    if value.is_empty() {
        "127.0.0.1:8787"
    } else {
        value
    }
    .parse()
}

fn parse_database(value: &str) -> Result<std::path::PathBuf, std::convert::Infallible> {
    Ok(if value.is_empty() {
        "./data/moleapi.db"
    } else {
        value
    }
    .into())
}

fn parse_bool<const DEFAULT: bool>(value: &str) -> Result<bool, std::str::ParseBoolError> {
    if value.is_empty() {
        Ok(DEFAULT)
    } else {
        value.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    fn parse(arguments: &[&str]) -> Result<Args, clap::Error> {
        // Isolate CLI tests from deployment variables on the test runner.
        let mut command = Args::command();
        for name in [
            "bind",
            "database_url",
            "database",
            "setup_token",
            "allow_private_network",
            "allow_registration",
        ] {
            command = command.mut_arg(name, |arg| arg.env(None::<&str>));
        }
        Args::from_arg_matches(&command.try_get_matches_from(arguments)?)
    }

    #[test]
    fn startup_defaults() {
        let args = parse(&["moleapi-server"]).unwrap();
        assert_eq!(args.bind.to_string(), "127.0.0.1:8787");
        assert_eq!(args.database, std::path::PathBuf::from("./data/moleapi.db"));
        assert!(args.database_url.is_none());
        assert!(args.setup_token.is_none());
        assert!(args.allow_private_network);
        assert!(!args.allow_registration);
    }

    #[test]
    fn boolean_flags_allow_missing_values() {
        let args = parse(&[
            "moleapi-server",
            "--allow-private-network",
            "--allow-registration",
        ])
        .unwrap();
        assert!(args.allow_private_network);
        assert!(args.allow_registration);
    }

    #[test]
    fn boolean_flags_accept_explicit_values() {
        for value in ["true", "false"] {
            let args = parse(&[
                "moleapi-server",
                "--allow-private-network",
                value,
                "--allow-registration",
                value,
            ])
            .unwrap();
            assert_eq!(args.allow_private_network, value == "true");
            assert_eq!(args.allow_registration, value == "true");
            let args = parse(&[
                "moleapi-server",
                &format!("--allow-private-network={value}"),
                &format!("--allow-registration={value}"),
            ])
            .unwrap();
            assert_eq!(args.allow_private_network, value == "true");
            assert_eq!(args.allow_registration, value == "true");
        }
    }

    #[test]
    fn valueless_startup_options_use_defaults() {
        let args = parse(&[
            "moleapi-server",
            "--bind",
            "--database",
            "--database-url",
            "--setup-token",
        ])
        .unwrap();
        assert_eq!(args.bind.to_string(), "127.0.0.1:8787");
        assert_eq!(args.database, std::path::PathBuf::from("./data/moleapi.db"));
        assert!(args.database_url.is_none());
        assert!(args.setup_token.is_none());
    }

    #[test]
    fn empty_values_use_defaults() {
        let args = parse(&[
            "moleapi-server",
            "--bind=",
            "--database=",
            "--allow-private-network=",
            "--allow-registration=",
        ])
        .unwrap();
        assert_eq!(args.bind.to_string(), "127.0.0.1:8787");
        assert_eq!(args.database, std::path::PathBuf::from("./data/moleapi.db"));
        assert!(args.allow_private_network);
        assert!(!args.allow_registration);
    }

    #[test]
    fn explicit_startup_values_are_preserved() {
        let args = parse(&[
            "moleapi-server",
            "--bind=0.0.0.0:9999",
            "--database=/tmp/custom.db",
            "--database-url=sqlite::memory:",
            "--setup-token=custom-token",
        ])
        .unwrap();
        assert_eq!(args.bind.to_string(), "0.0.0.0:9999");
        assert_eq!(args.database, std::path::PathBuf::from("/tmp/custom.db"));
        assert_eq!(args.database_url.as_deref(), Some("sqlite::memory:"));
        assert_eq!(args.setup_token.as_deref(), Some("custom-token"));
    }

    #[test]
    fn invalid_explicit_values_are_rejected() {
        for arguments in [
            vec!["moleapi-server", "--allow-private-network=invalid"],
            vec!["moleapi-server", "--allow-registration=invalid"],
            vec!["moleapi-server", "--bind=invalid"],
        ] {
            assert!(parse(&arguments).is_err());
        }
    }
}
