use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        run_loop()?;
    } else {
        match args.first().context("err")?.as_str() {
            "a" => print_all_w(None)?,
            "c" => clear_all()?,
            "r" => {
                if let Some(word_to_remove) = args.get(1) {
                    let mut words = Words::load_words()?;
                    if words.remove_word(word_to_remove) {
                        words.save_words()?;
                        println!("Removed");
                    } else {
                        println!("Not found");
                    }
                }
            }

            _ => {
                println!("usage:\na - print all words\nc - clear all words\nr - remove word")
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
    fn load_words() -> Result<Self> {
        let data_path = get_path()?;
        if !data_path.exists() {
            return Ok(Words { words: vec![] });
        }
        let content = fs::read_to_string(&data_path).context("faled to read data file")?;
        let worlds = toml::from_str(&content).context("faled to parse string to toml")?;
        Ok(worlds)
    }
    fn save_words(&self) -> Result<()> {
        let data_path = get_path()?;
        let string_content =
            toml::to_string_pretty(&self).context("faled to prse toml to string")?;
        fs::write(&data_path, &string_content).context("faled to write data")?;
        Ok(())
    }
    fn add_word(&mut self, word: String) {
        self.words.push(word);
    }
}

fn get_path() -> Result<PathBuf> {
    let path_folder = dirs::data_local_dir()
        .context("folder not found")?
        .join("words_saver");
    fs::create_dir_all(&path_folder)?;
    Ok(path_folder.join("data.toml"))
}
fn print_all_w(w: Option<&Words>) -> Result<()> {
    let words = match w {
        None => &Words::load_words()?,
        Some(wr) => wr,
    };
    println!("\n\n");
    for word in &words.words {
        println!("{}", word);
    }
    println!("\n\n");

    Ok(())
}
fn clear_all() -> Result<()> {
    let w = Words { words: vec![] };
    w.save_words()?;
    Ok(())
}

fn run_loop() -> Result<()> {
    let mut words = Words::load_words()?;
    loop {
        let user_input = input("q! to exit and save a! to print all: ")?;
        if user_input.is_empty() {
            continue;
        }
        match user_input.as_str() {
            "q!" => {
                words.save_words()?;
                break;
            }
            "a!" => print_all_w(Some(&words))?,
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
