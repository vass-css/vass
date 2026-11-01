//! Plugin command

use clap::{Args as ClapArgs, Subcommand};

/// Manage plugins
#[derive(ClapArgs, Debug)]
pub struct Args {
  #[command(subcommand)]
  command: PluginCommands,
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
  /// List installed plugins
  List,
  /// Install a plugin
  Install { name: String },
  /// Remove a plugin
  Remove { name: String },
  /// Update plugins
  Update,
}

/// Run the plugin command
pub async fn run(args: Args) -> anyhow::Result<()> {
  match args.command {
    PluginCommands::List => {
      println!("Installed plugins:");
      println!("  (none)");
    }
    PluginCommands::Install { name } => {
      println!("Installing plugin '{}'...", name);
      println!("Plugin system not yet implemented.");
    }
    PluginCommands::Remove { name } => {
      println!("Removing plugin '{}'...", name);
      println!("Plugin system not yet implemented.");
    }
    PluginCommands::Update => {
      println!("Updating plugins...");
      println!("Plugin system not yet implemented.");
    }
  }

  Ok(())
}
