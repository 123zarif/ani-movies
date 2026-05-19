use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Body {
    #[serde(rename = "cSearch")]
    c_search: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Show {
    id: String,
    name: String,
    image: String,
}

#[derive(Debug, Clone)]
struct Episode {
    title: String,
    url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HistoryItem {
    show_id: String,
    show_name: String,
    episode_title: String,
    url: String,
}

fn get_history_path() -> PathBuf {
    let config_dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config")
    };

    let app_dir = config_dir.join("ani-movies");
    let _ = std::fs::create_dir_all(&app_dir);
    app_dir.join("history.json")
}

fn load_history() -> Vec<HistoryItem> {
    let path = get_history_path();
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_else(|_| Vec::new())
    } else {
        Vec::new()
    }
}

fn save_history(new_item: HistoryItem) -> Result<(), Box<dyn Error>> {
    let mut history = load_history();

    if let Some(existing) = history
        .iter_mut()
        .find(|item| item.show_id == new_item.show_id)
    {
        existing.episode_title = new_item.episode_title;
        existing.url = new_item.url;
        existing.show_name = new_item.show_name;
    } else {
        history.push(new_item);
    }

    let path = get_history_path();
    let file = File::create(&path)?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &history)?;

    Ok(())
}

fn select_history_with_fzf(history: &[HistoryItem]) -> Result<Option<HistoryItem>, Box<dyn Error>> {
    if history.is_empty() {
        return Ok(None);
    }

    let mut fzf_input = String::new();
    for item in history {
        fzf_input.push_str(&format!("{} - {}\n", item.show_name, item.episode_title));
    }

    let mut child = Command::new("fzf")
        .arg("--prompt=Continue watching: ")
        .arg("--layout=reverse")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start fzf.");

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(fzf_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selection.is_empty() {
        return Ok(None);
    }

    if let Some(selected_item) = history
        .iter()
        .find(|h| format!("{} - {}", h.show_name, h.episode_title) == selection)
    {
        return Ok(Some(selected_item.clone()));
    }

    Ok(None)
}

fn continue_watching() -> Result<(), Box<dyn Error>> {
    let history = load_history();

    if history.is_empty() {
        println!("Your watch history is empty. Go watch something first!");
        return Ok(());
    }

    match select_history_with_fzf(&history)? {
        Some(selected_item) => {
            println!(
                "\nResuming: {} - {}",
                selected_item.show_name, selected_item.episode_title
            );

            if let Err(e) = play_video(&selected_item.url) {
                eprintln!("Failed to play video: {}", e);
            }
        }
        None => {
            println!("\nResume canceled.");
        }
    }

    Ok(())
}

fn play_video(url: &str) -> Result<(), Box<dyn Error>> {
    let is_termux = env::var("TERMUX_VERSION").is_ok();
    let mut player;

    if is_termux {
        println!("Detected Termux. Launching Android player...");
        player = Command::new("termux-open");
        player.arg(url);
    } else {
        println!("Desktop environment detected. Launching mpv...");
        player = Command::new("mpv");
        player.arg("--save-position-on-quit");
        player.arg(url);
    }

    player.spawn()?.wait()?;
    Ok(())
}

fn select_show_with_fzf(shows: &[Show]) -> Result<Option<Show>, Box<dyn Error>> {
    if shows.is_empty() {
        return Ok(None);
    }

    let mut fzf_input = String::new();
    for show in shows {
        fzf_input.push_str(&format!("{} | {}\n", show.id, show.name));
    }

    let mut child = Command::new("fzf")
        .arg("--prompt=Select a show: ")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start fzf. Is it installed?");

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(fzf_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selection.is_empty() {
        return Ok(None);
    }

    if let Some(selected_id) = selection.split(" | ").next() {
        if let Some(selected_show) = shows.iter().find(|s| s.id == selected_id) {
            return Ok(Some(selected_show.clone()));
        }
    }

    Ok(None)
}

async fn scrape_episodes(play_id: &str) -> Result<Vec<Episode>, Box<dyn Error>> {
    let url = format!("http://10.16.100.244/player.php?play={}", play_id);
    println!("Fetching player page: {}\n", url);

    let html_content = reqwest::get(&url).await?.text().await?;

    let document = Html::parse_document(&html_content);
    let source_selector = Selector::parse("video#video-id source").unwrap();

    let mut episodes = Vec::new();

    for element in document.select(&source_selector) {
        if let (Some(title), Some(src)) =
            (element.value().attr("title"), element.value().attr("src"))
        {
            episodes.push(Episode {
                title: title.to_string(),
                url: src.to_string(),
            });
        }
    }

    Ok(episodes)
}

fn select_episode_with_fzf(episodes: &[Episode]) -> Result<Option<Episode>, Box<dyn Error>> {
    if episodes.is_empty() {
        return Ok(None);
    }

    let mut fzf_input = String::new();
    for ep in episodes {
        fzf_input.push_str(&format!("{}\n", ep.title));
    }

    let mut child = Command::new("fzf")
        .arg("--prompt=Select an episode: ")
        .arg("--layout=reverse")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start fzf.");

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(fzf_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selection.is_empty() {
        return Ok(None);
    }

    if let Some(selected_ep) = episodes.iter().find(|e| e.title == selection) {
        return Ok(Some(selected_ep.clone()));
    }

    Ok(None)
}

async fn fetch_shows(query: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let url = "http://10.16.100.244/command.php";
    let body = Body {
        c_search: query.to_string(),
    };

    let fetch = client.post(url).form(&body).send().await?;

    if fetch.status().is_success() {
        let shows: Vec<Show> = fetch.json().await?;

        match select_show_with_fzf(&shows)? {
            Some(selected_show) => {
                let episodes = scrape_episodes(&selected_show.id).await?;

                if episodes.is_empty() {
                    println!("No video links found for this show.");
                    return Ok(());
                }

                match select_episode_with_fzf(&episodes)? {
                    Some(selected_ep) => {
                        println!("\nPreparing to play: {}", selected_ep.title);

                        let current_watch = HistoryItem {
                            show_id: selected_show.id.clone(),
                            show_name: selected_show.name.clone(),
                            episode_title: selected_ep.title.clone(),
                            url: selected_ep.url.clone(),
                        };

                        if let Err(e) = save_history(current_watch) {
                            eprintln!("Warning: Failed to save history: {}", e);
                        } else {
                            println!("Progress saved to ~/.config/ani-movies/history.json!");
                        }

                        if let Err(e) = play_video(&selected_ep.url) {
                            eprintln!("Failed to play video: {}", e);
                        }
                    }
                    None => println!("\nEpisode selection canceled."),
                }
            }
            None => {
                println!("\nNo show was selected. Exiting...");
            }
        }
    } else {
        eprintln!("Request failed with status code: {}", fetch.status());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <query>", args[0]);
        eprintln!("       {} -c (to continue watching)", args[0]);
        std::process::exit(1);
    }

    let input = &args[1];

    if input == "-c" {
        if let Err(e) = continue_watching() {
            eprintln!("Application Error: {}", e);
        }
    } else {
        if let Err(e) = fetch_shows(input).await {
            eprintln!("Application Error: {}", e);
        }
    }
}
