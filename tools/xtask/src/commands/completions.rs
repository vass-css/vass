//! `cargo xtask completions` — generate shell completion scripts to disk.

use clap_complete::{Shell, generate};

pub fn run(shell: Shell) -> anyhow::Result<()> {
  let mut cmd = clap::Command::new("vass")
    .about("VASS CSS preprocessor CLI")
    .subcommand(clap::Command::new("compile").about("Compile a VASS file to CSS"))
    .subcommand(clap::Command::new("build").about("Build all stylesheets in a directory"))
    .subcommand(clap::Command::new("check").about("Check files for errors"))
    .subcommand(clap::Command::new("fmt").about("Format VASS source files"))
    .subcommand(clap::Command::new("lint").about("Lint files for errors"))
    .subcommand(clap::Command::new("watch").about("Watch files for changes"))
    .subcommand(clap::Command::new("lsp").about("Start the LSP server"))
    .subcommand(clap::Command::new("doctor").about("Diagnose the VASS installation"))
    .subcommand(clap::Command::new("completions").about("Generate shell completions"))
    .subcommand(clap::Command::new("version").about("Print detailed version information"));

  std::fs::create_dir_all("completions")?;
  let name = shell.to_string().to_lowercase();
  let path = std::path::Path::new("completions").join(format!("vass.{name}"));
  let mut file = std::fs::File::create(&path)?;
  generate(shell, &mut cmd, "vass", &mut file);
  println!("wrote {}", path.display());
  Ok(())
}
