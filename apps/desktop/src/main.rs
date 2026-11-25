use std::path::PathBuf;

use phantom_desktop::headless;
use phantom_desktop::infra::paths;

const USAGE: &str = "phantom — the Phantom browser desktop shell

usage:
  phantom [--profile-dir <path>] [--headless] [--version]

options:
  --profile-dir <path>  override the profile directory (default: platform data dir)
  --headless            restore and report session state, then exit
  --version             print the version and exit";

fn main() {
    let mut profile = paths::profile_dir();
    let mut headless_mode = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => headless_mode = true,
            "--version" => {
                println!("phantom {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--profile-dir" => match args.next() {
                Some(dir) => profile = PathBuf::from(dir),
                None => {
                    eprintln!("phantom: --profile-dir needs a path\n{USAGE}");
                    std::process::exit(2);
                }
            },
            "--help" | "-h" => {
                println!("{USAGE}");
                return;
            }
            other => {
                eprintln!("phantom: unknown option '{other}'\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    if headless_mode {
        std::process::exit(headless::run(&profile));
    }

    #[cfg(feature = "ui-shell")]
    {
        phantom_desktop::ui::launch(profile);
    }
    #[cfg(not(feature = "ui-shell"))]
    {
        eprintln!("phantom: this build has no ui-shell feature; running headless instead");
        std::process::exit(headless::run(&profile));
    }
}
