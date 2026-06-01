use anyhow::bail;
use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    ChangeDeck {
        #[arg(required = true)]
        name: Vec<String>,
    },

    /// delite deck toml or -rs
    #[command(alias = "-rd")]
    RemoveDeck {
        #[arg(required = true)]
        name: Vec<String>,
    },

    /// clear all words or -c
    #[command(alias = "-c")]
    Clear,

    /// remove word or -r
    #[command(alias = "-r")]
    Remove {
        #[arg(required = true)]
        word: Vec<String>,
    },

    /// add word if dont want to run loop
    Add {
        #[arg(required = true)]
        word: Vec<String>,
    },

    #[command(alias = "-n")]
    /// create a new deck or -n
    New {
        #[arg(required = true)]
        name: Vec<String>,
    },

    #[command(alias = "-l")]
    /// listen to the clipboard and automatically add words from the clipboard or -l
    Listen,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Config {
    active_deck: String,
    loop_message: String,
    auto_switch: bool,
    seconds_check_clipboard: f32,
    auto_save_time: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            active_deck: "default".to_string(),
            loop_message: "enter a word or q! - exit and save a! - print all words > ".to_string(),
            auto_switch: true,
            seconds_check_clipboard: 1.5,
            auto_save_time: 3.0,
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
        let cfg: Config = toml::from_str(&content).context("failed to parse config")?;
        if !content.contains("auto_save_time") {
            cfg.save()?;
            println!("[Migration] Config file updated to the latest version.");
        }

        Ok(cfg)
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
            )
                .replace("auto_save_time =", "# Automatic saving during listen mode in minuts\nauto_save_time=");

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
            Command::All => {
                let words = Words::load_words(current)?;
                print_all_w(&words, current);
            }
            Command::Clear => clear_all(current)?,
            Command::Add { word } => {
                let final_phase = word.join(" ");
                let mut words = Words::load_words(current)?;
                if words.words.contains(&final_phase) {
                    bail!("Already exists");
                }
                words.add_word(final_phase);
                words.save_words(current)?;
            }
            Command::Remove { word } => {
                let mut words = Words::load_words(current)?;
                let final_phrase = word.join(" ").to_lowercase();
                if words.remove_word(&final_phrase) {
                    words.save_words(current)?;
                    println!("Removed");
                } else {
                    println!("Not found");
                }
            }
            Command::New { name } => {
                let final_phrase = name.join(" ");
                let path = get_deck_path(&final_phrase)?;
                if path.exists() {
                    println!("Error: Deck '{}' already exists!", &final_phrase);
                } else {
                    let words = Words { words: Vec::new() };
                    words.save_words(&final_phrase)?;
                    println!("Deck: {} created", &final_phrase);
                    if config.auto_switch {
                        config.active_deck = final_phrase.clone();
                        println!("Switched to deck: {}", &final_phrase);
                        config.save()?;
                    }
                }
            }
            Command::ChangeDeck { name } => {
                let final_phrase = name.join(" ");
                let path = get_deck_path(&final_phrase)?;
                if !path.exists() {
                    println!(
                        "Deck '{}' does not exist. Use -n to create it.",
                        &final_phrase
                    );
                } else {
                    config.active_deck = final_phrase.clone();
                    config.save()?;
                    println!("Switched to deck '{}'.", &final_phrase);
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
                let final_pharse = name.join(" ");
                let path = get_deck_path(&final_pharse)?;
                if !path.exists() {
                    bail!("deck doesnt exist")
                }
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove: {}", &path.display()))?;
                println!("Deck: {} removed", &final_pharse);
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
        self.words.push(word.trim().to_lowercase());
    }
}

fn get_deck_path(name: &str) -> Result<PathBuf> {
    let path_folder = dirs::data_local_dir()
        .context("folder not found")?
        .join("words_saver");
    fs::create_dir_all(&path_folder)?;
    Ok(path_folder.join(format!("{}.toml", name,)))
}
fn print_all_w(words: &Words, deck_name: &str) {
    println!(
        "\nDeck: {} | Word count: {}\n",
        deck_name,
        words.words.len()
    );

    for word in &words.words {
        println!("{}", word);
    }

    println!("\n\n");
}
fn clear_all(deck_name: &str) -> Result<()> {
    let w = Words { words: vec![] };
    w.save_words(deck_name)?;
    Ok(())
}
fn get_clipboard_text(clipboard: &mut Clipboard) -> Result<String> {
    let text = clipboard
        .get_text()
        .context("failed to get clipboard text")?;
    Ok(text.trim().to_lowercase())
}

fn listen_to_clipboard(config: &Config) -> Result<()> {
    let current_deck = &config.active_deck;
    let words: Arc<Mutex<Words>> = Arc::new(Mutex::new(Words::load_words(current_deck)?));
    let is_running = Arc::new(AtomicBool::new(true));
    let words_thread = Arc::clone(&words);
    let is_runnig_thread = Arc::clone(&is_running);
    let cfg = config.clone();

    println!("Active deck: {}", current_deck);

    let mut clipboard = Clipboard::new().context("Couldnt connect to the clipboard")?;
    let thread_handle = thread::spawn(move || {
        let mut last_copied = get_clipboard_text(&mut clipboard).unwrap_or_default();
        let mut last_save = Instant::now();
        while is_runnig_thread.load(Ordering::Relaxed) {
            if let Ok(clipboard_text) = get_clipboard_text(&mut clipboard)
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
            let last_save_time_min = last_save.elapsed().as_secs_f32() / 60.0;
            if last_save_time_min > cfg.auto_save_time {
                let words_guard = words_thread.lock().unwrap();
                match words_guard.save_words(&cfg.active_deck) {
                    Ok(_) => {
                        println!("Auto save: {}", &cfg.active_deck);
                        last_save = Instant::now()
                    }
                    Err(e) => {
                        eprintln!("[Auto-save error] {}", e);
                    }
                }
            }

            std::thread::sleep(Duration::from_secs_f32(cfg.seconds_check_clipboard));
        }
    });
    loop {
        let user_input = input("enter q! to stop > \n")?;

        match user_input.as_str() {
            "q!" => {
                is_running.store(false, Ordering::Relaxed);
                break;
            }
            "a!" => {
                let words_guard = words.lock().unwrap();
                print_all_w(&words_guard, current_deck);
            }

            _ => {
                println!("Unknown command :\nq! - to exit\na! - print all words")
            }
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
            "a!" => {
                print_all_w(&words, current_deck);
            }
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
    Ok(s.trim().to_string().to_lowercase())
}
