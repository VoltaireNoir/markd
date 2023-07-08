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
use tabled::{builder::Builder, settings::Style};

const DB_PATH: Lazy<PathBuf> = Lazy::new(db_path);

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
        #[arg(short, long, default_value_t = false, help = "Order list by paths")]
        path: bool,
    },
    #[command(alias = "p", about = "Purge all bookmarks whose paths no longer exist")]
    Purge,
    #[command(
        alias = "g",
        about = "Get bookmark's path (use with cd and command substitution)"
    )]
    Get { bookmark: String },
    #[command(alias = "r", about = "Remove given directory entry from bookmarks")]
    Remove { bookmark: String },
}

fn main() -> Result<()> {
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
            } => list(&bookmarks, Filters { filter, start, end }, path),
            Commands::Purge => purge(&mut bookmarks)?,
            Commands::Get { bookmark } => get(&bookmarks, &bookmark)?,
            Commands::Remove { bookmark } => remove(&mut bookmarks, &bookmark)?,
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

    let msg = match bookmarks.get_mut(&name.to_lowercase()) {
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
            bookmarks.insert(name.to_lowercase(), path);
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

fn list(bookmarks: &HashMap<String, String>, filters: Filters, order_by_path: bool) {
    println!("{}", "Bookmarked directories:".green().bold());
    let mut table = new_table();
    let mut bookmarks: Vec<_> = bookmarks.iter().collect();
    bookmarks.sort_by_key(|(name, path)| if order_by_path { *path } else { *name });
    if filters.any() {
        filtered_list(table, &mut bookmarks, filters);
    } else {
        bookmarks.iter().enumerate().for_each(|(i, (name, path))| {
            table.push_record([&(i + 1).to_string(), name, path]);
        });
        print_table(table);
    }
}

fn filtered_list(mut table: Builder, bookmarks: &mut Vec<(&String, &String)>, filters: Filters) {
    if let Some(filter) = filters.filter.as_ref() {
        bookmarks.retain(|(name, _)| name.contains(filter));
    }
    if let Some(start) = filters.start.as_ref() {
        bookmarks.retain(|(name, _)| name.starts_with(start));
    }
    if let Some(end) = filters.end.as_ref() {
        bookmarks.retain(|(name, _)| name.ends_with(end));
    }
    bookmarks.iter().enumerate().for_each(|(i, (k, v))| {
        table.push_record([&(i + 1).to_string(), k, v]);
    });
    print_table(table);
}

fn new_table() -> Builder {
    let mut table = Builder::new();
    table.set_header(["Index", "Name", "Path"]);
    table
}

fn print_table(table: Builder) {
    println!("{}", table.build().with(Style::modern()).to_string());
}

fn get(bookmarks: &HashMap<String, String>, bookmark: &str) -> Result<()> {
    let path = bookmarks
        .get(bookmark)
        .with_context(|| format!("{} is not in bookmarks", bookmark))?;
    print!("{path}");
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
    for (i, entry) in to_remove.iter().enumerate() {
        let path = bookmarks.remove(entry).unwrap();
        table.push_record([&(i + 1).to_string(), entry, &path]);
    }
    print_table(table);
    save_bookmarks(bookmarks)?;
    Ok(())
}

fn load_bookmarks() -> Result<HashMap<String, String>> {
    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .open(&*DB_PATH)
        .context("failed to open $HOME/.bookmarks.json")?;
    let mut raw = String::new();
    file.read_to_string(&mut raw).unwrap();
    let bookmarks = if !raw.is_empty() {
        serde_json::from_str(&raw).context("failed to parse $HOME/.bookmarks.json")?
    } else {
        HashMap::new()
    };
    Ok(bookmarks)
}

fn save_bookmarks(bookmarks: &HashMap<String, String>) -> Result<()> {
    let json = serde_json::to_string_pretty(bookmarks).context("failed to serialize data")?;
    std::fs::write(&*DB_PATH, json).context("failed to write to bookmarks.json")?;
    Ok(())
}

fn db_path() -> PathBuf {
    let mut home = home_dir().expect("failed to get home directory");
    home.push("bookmarks.json");
    home
}

fn panic_hook(info: &PanicInfo) {
    eprintln!("{} {}", "Error:".red().bold(), info)
}
