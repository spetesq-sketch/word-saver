use anyhow::bail;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "Ws", version, about = "a tiny program to keep words")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// print all words or -a
    #[command(alias = "-a")]
    All,

    #[command(alias = "-ad")]
    /// print all decks or -as
    AllDecks,
    #[command(alias = "-cd")]
    /// change active deck
    ChangeDeck { name: String },

    /// delite deck toml or -rs
    #[command(alias = "-rd")]
    RemoveDeck { name: String },

    /// clear all words or -c
    #[command(alias = "-c")]
    Clear,

    /// remove word or -r
    #[command(alias = "-r")]
    Remove { word: String },

    /// add word if dont want to run loop
    Add { word: Vec<String> },

    #[command(alias = "-n")]
    /// create a new deck or -n
    New { name: String },

    #[command(alias = "-l")]
    /// listen to the clipboard and automatically add words from the clipboard or -l
    Listen,
}

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    active_deck: String,
    loop_message: String,
    auto_switch: bool,
    seconds_check_clipboard: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            active_deck: "default".to_string(),
            loop_message: "enter a word or q! - exit and save a! - print all words > ".to_string(),
            auto_switch: true,
            seconds_check_clipboard: 1.5,
        }
    }
}
impl Config {
    fn get_path() -> Result<PathBuf> {
        let path = dirs::config_dir()
            .context("folder not found")?
            .join("words_saver");
        fs::create_dir_all(&path)?;
        Ok(path.join("config.toml"))
    }

    fn load() -> Result<Self> {
        let path = Self::get_path()?;
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let content = fs::read_to_string(&path).context("failed to read config")?;
        toml::from_str(&content).context("failed to parse config")
    }

    fn save(&self) -> Result<()> {
        let path = Self::get_path()?;
        let content = toml::to_string_pretty(self)?;

        let commented_content = content
            .replace(
                "active_deck =",
                "# The current active deck where all words are saved\nactive_deck ="
            )
            .replace(
                "loop_message =",
                "# Prompt for interactive input (ws)\nloop_message ="
            )
            .replace(
                "auto_switch =",
                "# Automatically switch to the newly created deck (true/false)\nauto_switch ="
            )
            .replace(
                "seconds_check_clipboard =",
                "# Clipboard check interval in listen mode (-l) in seconds\nseconds_check_clipboard ="
            );

        fs::write(&path, commented_content)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut config = Config::load()?;
    let current = &config.active_deck;

    if args.is_empty() {
        run_loop(&config)?;
    } else {
        let cli = Cli::parse();
        match cli.command {
            Command::All => print_all_w(None, current)?,
            Command::Clear => clear_all(current)?,
            Command::Add { word } => {
                let final_phase = word.join(" ");
                let mut words = Words::load_words(current)?;
                words.add_word(final_phase);
                words.save_words(current)?;
            }
            Command::Remove { word } => {
                let mut words = Words::load_words(current)?;
                if words.remove_word(&word) {
                    words.save_words(current)?;
                    println!("Removed");
                } else {
                    println!("Not found");
                }
            }
            Command::New { name } => {
                let path = get_deck_path(&name)?;
                if path.exists() {
                    println!("Error: Deck '{}' already exists!", name);
                } else {
                    let words = Words { words: Vec::new() };
                    words.save_words(&name)?;
                    println!("Deck: {} created", name);
                    if config.auto_switch {
                        config.active_deck = name.clone();
                        println!("Switched to deck: {}", name);
                        config.save()?;
                    }
                }
            }
            Command::ChangeDeck { name } => {
                let path = get_deck_path(&name)?;
                if !path.exists() {
                    println!("Deck '{}' does not exist. Use -n to create it.", name);
                } else {
                    config.active_deck = name.clone();
                    config.save()?;
                    println!("Switched to deck '{}'.", name);
                }
            }
            Command::AllDecks => {
                let path = dirs::data_local_dir()
                    .context("failed to get local data dir")?
                    .join("words_saver");
                println!("\nAvailable decks:");
                if path.exists() {
                    for entry in fs::read_dir(path)? {
                        let entry = entry?;
                        let file_name = entry.file_name();
                        let name = file_name.to_string_lossy().replace(".toml", "");
                        if name == *current {
                            println!("* {} (active)", name);
                        } else {
                            println!("  {}", name);
                        }
                    }
                }
                println!();
            }
            Command::Listen => listen_to_clipboard(&config)?,
            Command::RemoveDeck { name } => {
                let path = get_deck_path(&name)?;
                if !path.exists() {
                    bail!("deck doesnt exist")
                }
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove: {}", &path.display()))?;
                println!("Deck: {} removed", name);
            }
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Words {
    words: Vec<String>,
}
impl Words {
    fn remove_word(&mut self, word_to_remove: &str) -> bool {
        let original_len = self.words.len();
        self.words.retain(|w| w != word_to_remove);
        self.words.len() < original_len
    }
    fn load_words(deck_name: &str) -> Result<Self> {
        let data_path = get_deck_path(deck_name)?;
        if !data_path.exists() {
            return Ok(Words { words: vec![] });
        }
        let content = fs::read_to_string(&data_path).context("failed to read data file")?;
        let words = toml::from_str(&content).context("failed to parse string to toml")?;
        Ok(words)
    }
    fn save_words(&self, deck_name: &str) -> Result<()> {
        let data_path = get_deck_path(deck_name)?;
        let string_content =
            toml::to_string_pretty(&self).context("failed to parse toml to string")?;
        fs::write(&data_path, &string_content).context("failed to write data")?;
        Ok(())
    }
    fn add_word(&mut self, word: String) {
        self.words.push(word);
    }
}

fn get_deck_path(name: &str) -> Result<PathBuf> {
    let path_folder = dirs::data_local_dir()
        .context("folder not found")?
        .join("words_saver");
    fs::create_dir_all(&path_folder)?;
    Ok(path_folder.join(format!("{}.toml", name,)))
}
fn print_all_w(w: Option<&Words>, deck_name: &str) -> Result<()> {
    let loaded;
    let words = match w {
        None => {
            loaded = Words::load_words(deck_name)?;
            &loaded
        }
        Some(wr) => wr,
    };
    println!(
        "\nDeck: {} | Word count: {}\n",
        deck_name,
        words.words.len()
    );
    for word in &words.words {
        println!("{}", word);
    }
    println!("\n\n");
    Ok(())
}
fn clear_all(deck_name: &str) -> Result<()> {
    let w = Words { words: vec![] };
    w.save_words(deck_name)?;
    Ok(())
}

#[cfg(target_os = "linux")]
use std::process::Command as CdmCommand;
#[cfg(target_os = "linux")]
fn get_clipboard_text() -> Option<String> {
    if let Ok(output) = CdmCommand::new("wl-paste").arg("-n").output() {
        if output.status.success() {
            if let Ok(text) = String::from_utf8(output.stdout) {
                Some(text.trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else if let Ok(output) = CdmCommand::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
    {
        if output.status.success() {
            if let Ok(text) = String::from_utf8(output.stdout) {
                Some(text.trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(not(target_os = "linux"))]
use arboard::Clipboard;
#[cfg(not(target_os = "linux"))]
fn get_clipboard_text() -> Option<String> {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            return Some(text.trim().to_string());
        }
    }
    None
}

fn listen_to_clipboard(config: &Config) -> Result<()> {
    let current_deck = &config.active_deck;
    let words: Arc<Mutex<Words>> = Arc::new(Mutex::new(Words::load_words(current_deck)?));
    let is_running = Arc::new(AtomicBool::new(true));
    let words_thread = Arc::clone(&words);
    let is_runnig_thread = Arc::clone(&is_running);
    let cfg = config.clone();

    println!("Active deck: {}", current_deck);

    let thread_handle = thread::spawn(move || {
        let mut last_copied = get_clipboard_text().unwrap_or_default();
        while is_runnig_thread.load(Ordering::Relaxed) {
            if let Some(clipboard_text) = get_clipboard_text()
                && !clipboard_text.is_empty()
                && !clipboard_text.contains('\n')
                && clipboard_text != last_copied
            {
                last_copied = clipboard_text.clone();

                let mut words_guard = words_thread.lock().unwrap();

                if !words_guard.words.contains(&clipboard_text) {
                    words_guard.add_word(clipboard_text.clone());
                    println!("Added: {}", clipboard_text);
                }
            }

            std::thread::sleep(Duration::from_secs_f32(cfg.seconds_check_clipboard));
        }
    });
    loop {
        let user_input = input("enter q! to stop > \n")?;

        if user_input.as_str() == "q!" {
            is_running.store(false, Ordering::Relaxed);
            break;
        }
    }
    thread_handle.join().unwrap();
    let words_final = words.lock().unwrap();
    words_final.save_words(current_deck)?;
    Ok(())
}

fn run_loop(config: &Config) -> Result<()> {
    let current_deck = &config.active_deck;
    let mut words = Words::load_words(current_deck)?;

    println!("Active deck: {}", current_deck);

    loop {
        let user_input = input(&config.loop_message)?;
        if user_input.is_empty() {
            continue;
        }
        match user_input.as_str() {
            "q!" => {
                words.save_words(current_deck)?;
                break;
            }
            "a!" => print_all_w(Some(&words), current_deck)?,
            _ => {
                if words.words.contains(&user_input) {
                    println!("already added");
                    continue;
                }
                words.add_word(user_input);
            }
        }
    }
    Ok(())
}

fn input(msg: &str) -> Result<String> {
    print!("{}", msg);
    stdout().flush()?;
    let mut s = String::new();
    stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}
