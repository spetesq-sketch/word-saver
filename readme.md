# Word saver

word saver or ws is a tiny CLI tool written in Rust, designed to help collect new foreign words while reading articles, watching videos or just surfing the Internet for subsequent addition, for example, to Anki.

---

## Installation

### 1. System Dependencies (Linux Only)

I couldn't figure out how to retrieve data from the clipboard on Wayland, so I used `std::process::Command` for Linux; therefore, please install `wl-clipboard` and `xclip`

```bash
# Arch Linux 
sudo pacman -S wl-clipboard xclip 

# Ubuntu / Debian 
sudo apt install wl-clipboard xclip

# Fedora
sudo dnf install wl-clipboard xclip

# openSUSE
sudo zypper install wl-clipboard xclip

# Void Linux
sudo xbps-install -Su wl-clipboard xclip
```

## 2. Install via Cargo

You can install it via cargo

``` bash
cargo install word_saver
```

Or build from source

- just download source code and run

``` bash
cargo build --release
```

---

## Commands

 You can see all available commands by `ws --help`

```bash
# Start background mode for reading the clipboard to automatically add words to the active deck
ws -l # or ws listen

# Start a loop to manually add words by simply copying and pasting
ws

# Print all words on the active board
ws -a # or ws all

# Print all decks
ws -ad # or ws all-decks

# Change active deck
ws -cd <name> # or ws change-deck <name>

# Delite deck toml
ws -rd <name> # or ws remove-deck <name>

# Delete all words in the current deck
ws -c # or ws clear

# Add word if dont want to run loop
ws add <word>

# Create a new deck
ws -n <name> # or ws new <name>

```

---

## Features

- **Background Clipboard Listener (`-l`, `listen`)**: Automatically copies words and saves them to your active deck
 `note : Currently, this works by polling the clipboard at regular intervals, I may later implement a daemon-based approach to detect when the clipboard contents have changed.`
- **Duplication**: Automatically ignores duplicates, empty lines, and texts containing \n
- **Decks**: Organize your vocabulary into different decks (e.g., `english`, `german`, `verbs`)
- **Config**: There is a minimalist config

```toml
# The current active deck where all words are saved
active_deck = "default"
# Prompt for interactive input (ws)
loop_message = "enter a word or q! - exit and save a! - print all words > "
# Automatically switch to the newly created deck (true/false)
auto_switch = true
# Clipboard check interval in listen mode (-l) in seconds
seconds_check_clipboard = 1.5
```

---

## File Locations

Config : `~/.config/words_saver`
Decks: `~/.local/share/words_saver`

---
If you find a bug, please report it
If you like the project, please give it a star
