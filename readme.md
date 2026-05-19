# Ani-Movies CLI

A lightning-fast, interactive terminal application built in Rust for searching, streaming, and managing your movie/tv-series watch history.

## 🚀 Features

* Interactive Search: Built-in fuzzy search (fzf) integration for selecting shows and episodes effortlessly.
* Smart History: Automatically saves your progress. Never lose track of where you left off.
* Resume Playback: Use the -c flag to open your history and instantly resume your last watched show/episode.
* Post-Play Loop: After an episode ends, use a built-in menu to jump to the Next, Previous, Replay, or Select a new episode without restarting the app.
* Cross-Platform: Native support for Arch Linux (via mpv) and Android/Termux (via termux-open).

## 🛠 Prerequisites

Ensure you have the following installed on your system:

* Rust/Cargo: Install Rust from rust-lang.org
* fzf: The fuzzy finder for the CLI menus.
    * Arch: sudo pacman -S fzf
    * Termux: pkg install fzf
* mpv: (Desktop only) for video playback.
    * Arch: sudo pacman -S mpv

## 📦 Installation
   `git clone https://github.com/123zarif/ani-movies.git`

   `cd ani-movies`
     
  `make install`

## 🎮 Usage

### Search and Watch
Browse for a show and select your episode:
ani-movies "The Big Bang Theory"

### Resume Watching
Continue exactly where you left off:

  `ani-movies -c`

## ⚙️ Configuration

Your watch history is managed automatically at:
~/.config/ani-movies/history.json

This follows the XDG Base Directory specification, keeping your home directory clean.

## ⚠️ Disclaimer
This tool uses custom scraping logic to interface with specific backend services. Ensure you have the rights to access the content you are streaming.

## 🛠 Tech Stack
* Language: Rust
* Async Runtime: Tokio
* HTTP Client: Reqwest
* UI/TUI: fzf (Fuzzy Finder)
* Serialization: Serde
* Scraping: Scraper