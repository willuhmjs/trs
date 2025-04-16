//! CLI handling

use std::io;
use clap::{Command, Arg, ArgAction};
use crate::trash::{move_to_trash, empty_trash, show_trash_contents, interactive_restore};

/// Run the application
pub fn run() -> io::Result<()> {
    let matches = create_cli().get_matches();

    let trash_dir = dirs::data_local_dir()
        .expect("Could not find local share directory")
        .join("trash");

    if let Some(default_files) = matches.get_many::<String>("default_file") {
        // Process multiple files for the default command
        for file in default_files {
            move_to_trash(file, &trash_dir)?;
        }
    } else {
        match matches.subcommand() {
            Some(("move", sub_m)) => {
                // Process multiple files for the move command
                if let Some(files) = sub_m.get_many::<String>("file") {
                    for file in files {
                        move_to_trash(file, &trash_dir)?;
                    }
                }
            }
            Some(("restore", _)) => {
                interactive_restore(&trash_dir)?;
            }
            Some(("empty", _)) => {
                empty_trash(&trash_dir)?;
            }
            Some(("show", _)) => {
                show_trash_contents(&trash_dir)?;
            }
            _ => {
                // Show the help page for invalid commands
                create_cli().print_help().expect("Failed to print help");
                println!();
            }
        }
    }

    Ok(())
}

/// Create the CLI
fn create_cli() -> Command {
    Command::new("Trash CLI")
        .version("1.0")
        .author("William Faircloth")
        .about("A CLI program to manage a trash folder")
        .arg(
            Arg::new("default_file")
                .help("Path to file(s) or directory(ies) to move to trash (when used without subcommands)")
                .required(false)
                .action(ArgAction::Append) // Allow multiple values
                .num_args(1..),            // Accept one or more arguments
        )
        .subcommand(
            Command::new("move")
                .about("Move files or directories to the trash")
                .arg(
                    Arg::new("file")
                        .required(true)
                        .action(ArgAction::Append) // Allow multiple values
                        .num_args(1..)             // Accept one or more arguments
                        .help("Path(s) to the file(s) or directory(ies) to move to trash")
                ),
        )
        .subcommand(
            Command::new("restore")
                .about("Interactively select and restore items from the trash to their original locations"),
        )
        .subcommand(
            Command::new("empty")
                .about("Permanently delete all items in the trash folder"),
        )
        .subcommand(
            Command::new("show")
                .about("Display a list of all items currently in the trash with their original paths"),
        )
}
