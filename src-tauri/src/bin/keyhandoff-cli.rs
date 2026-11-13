//! The `keyhandoff` command for terminals on Windows.
//!
//! The app itself is a Windows GUI program, so a terminal neither waits for
//! it nor sees its output. This small console program is installed as
//! `bin\keyhandoff.exe` (that folder goes on the user's PATH) and runs the
//! same code: `keyhandoff run -- python report.py` waits, prompts in the app,
//! and prints the program's masked output. Anything else opens the app,
//! passing its arguments along (`keyhandoff key.handoff`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => std::process::exit(keyhandoff_lib::run_command(&args[1..])),
        Some("-h" | "--help" | "help") => {
            println!("Usage:\n  keyhandoff run [--profile NAME] [--env FILE] -- COMMAND [ARGS...]\n  keyhandoff [FILE...]    open KeyHandoff");
        }
        _ => match app() {
            Some(app) => {
                if let Err(e) = std::process::Command::new(&app).args(&args).spawn() {
                    eprintln!("keyhandoff: couldn't open {}: {e}", app.display());
                    std::process::exit(1);
                }
            }
            None => {
                eprintln!("keyhandoff: KeyHandoff isn't installed next to this command.");
                std::process::exit(1);
            }
        },
    }
}

/// The app: installed one folder up (`<install>\KeyHandoff.exe`), or beside
/// this command in a build folder.
fn app() -> Option<std::path::PathBuf> {
    let me = std::env::current_exe().ok()?;
    let here = me.parent()?;
    let found = [here.parent(), Some(here)]
        .into_iter()
        .flatten()
        .flat_map(|dir| ["KeyHandoff.exe", "keyhandoff.exe"].map(|name| dir.join(name)))
        .find(|p| p.is_file() && !p.as_os_str().eq_ignore_ascii_case(me.as_os_str()));
    found
}
