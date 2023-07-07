use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use dirs::home_dir;
use once_cell::unsync::Lazy;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, Read},
    panic::PanicInfo,
    path::{Path, PathBuf},
};

const DB_PATH: Lazy<PathBuf> = Lazy::new(db_path);

#[derive(Parser)]
#[command(name = "Markd")]
#[command(author = "Maaz Ahmed <mzahmed95@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "Bookmark directories for easy directory-hopping", long_about = None)]
struct Cli {
    #[arg(long, short, help = "Optional directory path")]
    path: Option<PathBuf>,
    #[arg(long, short, help = "Alias to use instead of dir name")]
    alias: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "List all bookmarks")]
    List {
        #[arg(short, long, help = "Filter list by name fragment")]
        filter: Option<String>,
    },
    #[command(about = "Purge all bookmarks whose paths no longer exist")]
    Purge,
    #[command(about = "Get bookmark's path (use with cd and interpolation)")]
    Get { bookmark: String },
    #[command(about = "Remove given directory entry from bookmarks")]
    Remove { bookmark: String },
}

fn main() -> Result<()> {
    std::panic::set_hook(Box::new(panic_hook));
    let args = Cli::parse();
    let mut dirs = load_dirs()?;
    if let Some(cmd) = args.command {
        match cmd {
            Commands::List { filter } => list(&dirs, filter.as_ref()),
            Commands::Purge => purge(&mut dirs)?,
            Commands::Get { bookmark } => get(&dirs, &bookmark)?,
            Commands::Remove { bookmark } => remove(&mut dirs, &bookmark)?,
        }
    } else {
        mark(&mut dirs, args.path, args.alias)?;
    }
    Ok(())
}

fn mark(
    dirs: &mut HashMap<String, String>,
    path: Option<PathBuf>,
    alias: Option<String>,
) -> Result<()> {
    let dir = if let Some(dir) = path {
        dir.try_exists()
            .and_then(|_| dir.canonicalize())
            .context("Invalid path provied")?
    } else {
        std::env::current_dir().context("failed to determine current directory")?
    };
    let path = dir.to_string_lossy().to_string();

    let name = alias.unwrap_or(
        dir.file_name()
            .context("couldn't get dir name")?
            .to_string_lossy()
            .to_string(),
    );

    let msg = match dirs.get_mut(&name) {
        Some(val) => {
            if update() {
                val.clear();
                val.push_str(&path);
                "bookmark entry updated"
            } else {
                "bookmark operation cancelled"
            }
        }
        None => {
            dirs.insert(name.clone(), path);
            "bookmarked"
        }
    };
    save_dirs(&dirs)?;
    let prompt = if msg.contains("cancelled") {
        "info:".yellow().bold()
    } else {
        "Success:".green().bold()
    };
    println!("{} {} {}", prompt, name.magenta(), msg);
    Ok(())
}

fn update() -> bool {
    println!(
        "{} direcotry name already exists in bookmarks, would you like to update it?\nType y / yes to update, anything else to cancel.",
        "info:".yellow().bold(),
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

fn list(dirs: &HashMap<String, String>, filter: Option<&String>) {
    println!("{}", "Bookmarked directories:".green().bold());
    let dirs = dirs.iter();
    if let Some(filter) = filter {
        dirs.filter(|(name, _)| name.contains(filter))
            .enumerate()
            .for_each(|(i, (k, v))| println!("[{}] {k}: {v}", i + 1));
    } else {
        dirs.enumerate()
            .for_each(|(i, (k, v))| println!("[{}] {k}: {v}", i + 1));
    }
}

fn get(dirs: &HashMap<String, String>, bookmark: &str) -> Result<()> {
    let path = dirs
        .get(bookmark)
        .with_context(|| format!("{} is not in bookmarks", bookmark))?;
    print!("{path}");
    Ok(())
}

fn remove(dirs: &mut HashMap<String, String>, bookmark: &str) -> Result<()> {
    dirs.remove(bookmark)
        .with_context(|| format!("{} is not in bookmarks", bookmark))?;
    save_dirs(&dirs)?;
    println!(
        "{} {} {}",
        "Success:".green().bold(),
        bookmark.red(),
        "removed from bookmarks"
    );
    Ok(())
}

fn purge(dirs: &mut HashMap<String, String>) -> Result<()> {
    let mut to_remove = vec![];
    for (name, path) in dirs.iter() {
        let p: &Path = path.as_ref();
        if !p.is_dir() {
            to_remove.push(name.clone());
        }
    }
    if to_remove.is_empty() {
        return Ok(println!("{} Nothing to purge", "Info:".yellow().bold()));
    }
    println!("{}", "Purged entries:".magenta().bold());
    for (i, entry) in to_remove.iter().enumerate() {
        let path = dirs.remove(entry).unwrap();
        println!("[{}] {entry}: {path}", i + 1);
    }
    save_dirs(dirs)?;
    Ok(())
}

fn load_dirs() -> Result<HashMap<String, String>> {
    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .open(&*DB_PATH)
        .context("failed to open $HOME/.dirs.json")?;
    let mut raw = String::new();
    file.read_to_string(&mut raw).unwrap();
    let dirs = if !raw.is_empty() {
        serde_json::from_str(&raw).context("failed to parse $HOME/.dirs.json")?
    } else {
        HashMap::new()
    };
    Ok(dirs)
}

fn save_dirs(dirs: &HashMap<String, String>) -> Result<()> {
    let json = serde_json::to_string_pretty(dirs).context("failed to serialize data")?;
    std::fs::write(&*DB_PATH, json).context("failed to write to dirs.json")?;
    Ok(())
}

fn db_path() -> PathBuf {
    let mut home = home_dir().unwrap();
    home.push("dirs.json");
    home
}

fn panic_hook(info: &PanicInfo) {
    eprintln!("{} {}", "error:".red().bold(), info)
}
