use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};

pub fn parse() -> ArgMatches {
    Command::new("glp")
        .author("Hrdina Pavel <hrdina.pavel@gmail.com>")
        .about("Gitlab pipeline status for command line.")
        .arg(
            Arg::new("project")
                .short('p')
                .long("project")
                .action(ArgAction::Set)
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .action(ArgAction::Set)
                .value_parser(value_parser!(u8))
                .default_value(super::DEFAULT_LIMIT.to_string()),
        )
        .arg(
            Arg::new("finished")
                .short('f')
                .long("finished")
                .action(ArgAction::SetTrue),
        )
        .get_matches()
}
