use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use dirs::home_dir;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, Read},
    panic::PanicInfo,
    path::{Path, PathBuf},
};
use tabled::{builder::Builder, settings::Style};

static DB_PATH: Lazy<PathBuf> = Lazy::new(db_path);
const CLIPNAME: &str = "markd-temp";
const ZSH_BASH: &str = r"goto() {
    cd $(markd g $1);
}";
const FISH: &str = r"function goto
    cd $(markd g $argv)
end";
const POWERSHELL: &str = r"function goto([string]$Bookmark) {
    cd (markd g $Bookmark)
}";

#[derive(Parser)]
#[command(name = "Markd")]
#[command(author = "Maaz Ahmed <mzahmed95@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "Bookmark directories for easy directory-hopping", long_about = None)]
struct Cli {
    #[arg(long, short, help = "Optional directory path to bookmark")]
    path: Option<PathBuf>,
    #[arg(long, short, help = "Alias to use instead of dir name")]
    alias: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "l", about = "List all bookmarks")]
    List {
        #[arg(short, long, help = "Filter list by name fragment")]
        filter: Option<String>,
        #[arg(short, long, help = "Filter list by starting char or fragment")]
        start: Option<String>,
        #[arg(short, long, help = "Filter list by ending char or fragment")]
        end: Option<String>,
        #[arg(
            short,
            long,
            default_value_t = false,
            help = "Print info as plain text instead of a formatted table"
        )]
        plain: bool,
        #[arg(short, long, default_value_t = false, help = "Order list by paths")]
        path: bool,
    },
    #[command(alias = "p", about = "Purge all bookmarks whose paths no longer exist")]
    Purge,
    #[command(
        alias = "g",
        about = "Get bookmark's path (use with cd and command substitution)"
    )]
    Get {
        #[arg(default_value_t = String::from(CLIPNAME))]
        bookmark: String,
        #[arg(
            short,
            long,
            default_value_t = false,
            help = "Return current working directory on failure"
        )]
        failsafe: bool,
    },
    #[command(
        alias = "c",
        about = "Save temp entry for quick access with `markd get` or `goto`",
        long_about = "Save current or provided directory to 'markd-temp' entry for quick switching. The saved entry will be used when no bookmark name is provided to `markd get` command"
    )]
    Clip,
    #[command(alias = "r", about = "Remove given directory entry from bookmarks")]
    Remove { bookmark: String },
    #[command(
        alias = "s",
        about = "Generate required config for 'goto' command shell support"
    )]
    Shell { stype: Shell },
    #[command(
        about = "Migrate old bookmarks.json to the new bookmarks.toml",
        long_about = "markd now uses TOML format for storing bookmarks instead of the old JSON format. This command helps you migrate your old bookmarks to the new file.\nNote: This command will be removed in the future releases."
    )]
    Migrate,
}

#[derive(ValueEnum, Clone, Copy)]
enum Shell {
    Fish,
    Zsh,
    Bash,
    Powershell,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{} {err}", "Error:".red().bold());
        for (i, cause) in err.chain().skip(1).enumerate() {
            if i == 0 {
                eprintln!("\n{}", "Caused by:".yellow().bold());
            }
            eprintln!("({}) {cause}", i + 1);
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    std::panic::set_hook(Box::new(panic_hook));
    let args = Cli::parse();
    let mut bookmarks = load_bookmarks()?;
    if let Some(cmd) = args.command {
        match cmd {
            Commands::List {
                filter,
                start,
                end,
                path,
                plain,
            } => list(&bookmarks, Filters { filter, start, end }, path, plain),
            Commands::Purge => purge(&mut bookmarks)?,
            Commands::Get { bookmark, failsafe } => get(&bookmarks, &bookmark, failsafe)?,
            Commands::Clip => mark(&mut bookmarks, args.path, Some(CLIPNAME.into()))?,
            Commands::Remove { bookmark } => remove(&mut bookmarks, &bookmark)?,
            Commands::Shell { stype } => shell(stype),
            Commands::Migrate => migrate()?,
        }
    } else {
        mark(&mut bookmarks, args.path, args.alias)?;
    }
    Ok(())
}

fn mark(
    bookmarks: &mut HashMap<String, String>,
    path: Option<PathBuf>,
    alias: Option<String>,
) -> Result<()> {
    let dir = validate_or_default(path)?;
    let path = dir.to_string_lossy().to_string();
    let name = alias
        .unwrap_or(
            dir.file_name()
                .context("couldn't get dir name")?
                .to_string_lossy()
                .to_string(),
        )
        .to_lowercase();

    let msg = match bookmarks.get_mut(&name) {
        Some(val) => {
            if name == CLIPNAME || update() {
                val.clear();
                val.push_str(&path);
                "bookmark entry updated"
            } else {
                "bookmark operation cancelled"
            }
        }
        None => {
            bookmarks.insert(name.clone(), path);
            "bookmarked"
        }
    };
    save_bookmarks(&bookmarks)?;
    let prompt = if msg.contains("cancelled") {
        "Info:".yellow().bold()
    } else {
        "Success:".green().bold()
    };
    println!("{} {} {}", prompt, name.magenta(), msg);
    Ok(())
}

fn validate_or_default(path: Option<PathBuf>) -> Result<PathBuf> {
    let dir = if let Some(dir) = path {
        match dir.try_exists() {
            Ok(true) => dir
                .is_dir()
                .then_some(dir.canonicalize().context("failed to expand path")?)
                .context("provided path is not a directory")?,
            _ => bail!("invalid path provided"),
        }
    } else {
        std::env::current_dir().context("failed to determine current directory")?
    };
    Ok(dir)
}

fn update() -> bool {
    println!(
        "{} direcotry name already exists in bookmarks, would you like to update it?\n\nType y / yes to update, anything else to cancel.",
        "Info:".yellow().bold(),
    );
    let mut res = String::new();
    io::stdin()
        .read_line(&mut res)
        .expect("failed to read from standard input");
    match res.trim() {
        "y" | "yes" => true,
        _ => false,
    }
}

struct Filters {
    filter: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

impl Filters {
    fn any(&self) -> bool {
        [&self.filter, &self.start, &self.end]
            .iter()
            .any(|f| f.is_some())
    }
}

fn list(bookmarks: &HashMap<String, String>, filters: Filters, order_by_path: bool, plain: bool) {
    let mut table = new_table();
    let mut bookmarks: Vec<_> = bookmarks.iter().collect();
    bookmarks.sort_by_key(|(name, path)| if order_by_path { *path } else { *name });
    if filters.any() {
        filter_list(&mut bookmarks, filters);
    }
    if plain {
        return bookmarks
            .iter()
            .for_each(|(name, path)| println!("{name}:{path}"));
    }
    println!("{}", "Bookmarked directories:".green().bold());
    bookmarks.iter().for_each(|(name, path)| {
        table.push_record([*name, *path]);
    });
    print_table(table);
}

#[inline]
fn filter_list(bookmarks: &mut Vec<(&String, &String)>, filters: Filters) {
    if let Some(filter) = filters.filter.as_ref() {
        bookmarks.retain(|(name, _)| name.contains(filter));
    }
    if let Some(start) = filters.start.as_ref() {
        bookmarks.retain(|(name, _)| name.starts_with(start));
    }
    if let Some(end) = filters.end.as_ref() {
        bookmarks.retain(|(name, _)| name.ends_with(end));
    }
}

fn new_table() -> Builder {
    let mut table = Builder::new();
    table.set_header(["Name", "Path"]);
    table
}

fn print_table(table: Builder) {
    println!("{}", table.index().build().with(Style::rounded()));
}

fn get(bookmarks: &HashMap<String, String>, bookmark: &str, failsafe: bool) -> Result<()> {
    let path = bookmarks
        .get(bookmark)
        .with_context(|| format!("{} is not in bookmarks", bookmark));
    match path {
        Ok(path) => print!("{path}"),
        Err(err) => {
            if failsafe {
                let cwd =
                    std::env::current_dir().context("could not get current working directory")?;
                print!("{path}", path = cwd.display());
            }
            return Err(err);
        }
    }
    Ok(())
}

fn remove(bookmarks: &mut HashMap<String, String>, bookmark: &str) -> Result<()> {
    bookmarks
        .remove(bookmark)
        .with_context(|| format!("{} is not in bookmarks", bookmark))?;
    save_bookmarks(&bookmarks)?;
    println!(
        "{} {} {}",
        "Success:".green().bold(),
        bookmark.red(),
        "removed from bookmarks"
    );
    Ok(())
}

fn purge(bookmarks: &mut HashMap<String, String>) -> Result<()> {
    let mut to_remove = vec![];
    for (name, path) in bookmarks.iter() {
        let p: &Path = path.as_ref();
        if !p.is_dir() {
            to_remove.push(name.clone());
        }
    }
    if to_remove.is_empty() {
        return Ok(println!("{} Nothing to purge", "Info:".yellow().bold()));
    }
    println!("{}", "Purged bookmarks:".red().bold());
    let mut table = new_table();
    for entry in to_remove.iter() {
        let path = bookmarks.remove(entry).unwrap();
        table.push_record([entry, &path]);
    }
    print_table(table);
    save_bookmarks(bookmarks)?;
    Ok(())
}

fn load_bookmarks() -> Result<HashMap<String, String>> {
    let mut file = std::fs::File::options()
        .read(true)
        .create(true)
        .write(true)
        .open(DB_PATH.as_path())?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .context("failed to read $HOME/bookmarks.toml")?;
    Ok(toml::from_str(&raw).context("failed to parse $HOME/.bookmarks.toml")?)
}

fn save_bookmarks(bookmarks: &HashMap<String, String>) -> Result<()> {
    let toml = toml::to_string_pretty(bookmarks).context("failed to serialize data")?;
    std::fs::write(DB_PATH.as_path(), toml).context("failed to write to bookmarks.toml")?;
    Ok(())
}

fn db_path() -> PathBuf {
    let mut home = home_dir().expect("failed to get home directory");
    home.push("bookmarks.toml");
    home
}

fn panic_hook(info: &PanicInfo) {
    eprintln!("{} {}", "Error:".red().bold(), info)
}

fn shell(stype: Shell) {
    println!(
        "{}",
        match stype {
            Shell::Fish => FISH,
            Shell::Zsh | Shell::Bash => ZSH_BASH,
            Shell::Powershell => POWERSHELL,
        }
    )
}

fn migrate() -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .open(DB_PATH.with_file_name("bookmarks.json"))
        .context("failed to open $HOME/bookmarks.json")?;

    let old_data: HashMap<String, String> =
        serde_json::from_reader(file).context("failed to parse bookmarks.json")?;
    let toml =
        toml::to_string_pretty(&old_data).context("failed to convert old bookmarks to TOML")?;
    std::fs::write(DB_PATH.as_path(), toml).context("Failed to write to bookmarks.toml")?;
    println!("{} migration complete", "Success:".green().bold());
    Ok(())
}
