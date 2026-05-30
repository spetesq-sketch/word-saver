use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::PathBuf;

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

    #[command(alias = "-as")]
    /// print all sessions or -as
    AllSession,
    #[command(alias = "-cs")]
    /// change active session
    ChangeSession { name: String },

    /// clear all words or -c
    #[command(alias = "-c")]
    Clear,

    /// remove word or -r
    #[command(alias = "-r")]
    Remove { word: String },

    /// add word if dont want to run loop
    Add { word: String },

    #[command(alias = "-n")]
    /// create a new session
    New { name: String },
}

#[derive(Serialize, Deserialize)]
struct Config {
    active_session: String,
    loop_message: String,
    auto_switch: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            active_session: "default".to_string(),
            loop_message: "enter a word or q! - exit and save a! - print all words > ".to_string(),
            auto_switch: true,
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
        fs::write(&path, content)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut config = Config::load()?;

    if args.is_empty() {
        run_loop(&config)?;
    } else {
        let cli = Cli::parse();
        let current = &config.active_session;
        match cli.command {
            Command::All => print_all_w(None, current)?,
            Command::Clear => clear_all(current)?,
            Command::Add { word } => {
                let mut words = Words::load_words(current)?;
                words.add_word(word);
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
                let path = get_session_path(&name)?;
                if path.exists() {
                    println!("Error: Session '{}' already exists!", name);
                } else {
                    let words = Words { words: Vec::new() };
                    words.save_words(&name)?;
                    println!("Session: {} created", name);
                    if config.auto_switch {
                        config.active_session = name.clone();
                        println!("Switched to session: {}", name);
                        config.save()?;
                    }
                }
            }
            Command::ChangeSession { name } => {
                let path = get_session_path(&name)?;
                if !path.exists() {
                    println!("Session '{}' does not exist. Use -n to create it.", name);
                } else {
                    config.active_session = name.clone();
                    config.save()?;
                    println!("Switched to session '{}'.", name);
                }
            }
            Command::AllSession => {
                let path = dirs::data_local_dir()
                    .context("failed to get local data dir")?
                    .join("words_saver");
                println!("\nAvailable sessions:");
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
    fn load_words(session_name: &str) -> Result<Self> {
        let data_path = get_session_path(session_name)?;
        if !data_path.exists() {
            return Ok(Words { words: vec![] });
        }
        let content = fs::read_to_string(&data_path).context("failed to read data file")?;
        let words = toml::from_str(&content).context("failed to parse string to toml")?;
        Ok(words)
    }
    fn save_words(&self, session_name: &str) -> Result<()> {
        let data_path = get_session_path(session_name)?;
        let string_content =
            toml::to_string_pretty(&self).context("failed to parse toml to string")?;
        fs::write(&data_path, &string_content).context("failed to write data")?;
        Ok(())
    }
    fn add_word(&mut self, word: String) {
        self.words.push(word);
    }
}

fn get_session_path(name: &str) -> Result<PathBuf> {
    let path_folder = dirs::data_local_dir()
        .context("folder not found")?
        .join("words_saver");
    fs::create_dir_all(&path_folder)?;
    Ok(path_folder.join(format!("{}.toml", name,)))
}
fn print_all_w(w: Option<&Words>, session_name: &str) -> Result<()> {
    let loaded;
    let words = match w {
        None => {
            loaded = Words::load_words(session_name)?;
            &loaded
        }
        Some(wr) => wr,
    };
    println!(
        "\nSession: {} | Word count: {}\n",
        session_name,
        words.words.len()
    );
    for word in &words.words {
        println!("{}", word);
    }
    println!("\n\n");
    Ok(())
}
fn clear_all(session_name: &str) -> Result<()> {
    let w = Words { words: vec![] };
    w.save_words(session_name)?;
    Ok(())
}

fn run_loop(config: &Config) -> Result<()> {
    let current_session = &config.active_session;
    let mut words = Words::load_words(current_session)?;

    println!("Active session: {}", current_session);

    loop {
        let user_input = input(&config.loop_message)?;
        if user_input.is_empty() {
            continue;
        }
        match user_input.as_str() {
            "q!" => {
                words.save_words(current_session)?;
                break;
            }
            "a!" => print_all_w(Some(&words), current_session)?,
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
