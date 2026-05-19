use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Serialize)]
struct Body {
    #[serde(rename = "cSearch")]
    c_search: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Show {
    id: String,
    name: String,
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

enum PostPlayAction {
    Next,
    Prev,
    Replay,
    Episodes,
    Quit,
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
        .spawn()?;

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

async fn continue_watching() -> Result<(), Box<dyn Error>> {
    let history = load_history();

    if history.is_empty() {
        println!("Your watch history is empty.");
        return Ok(());
    }

    match select_history_with_fzf(&history)? {
        Some(selected_item) => {
            handle_show(
                &selected_item.show_id,
                &selected_item.show_name,
                Some(&selected_item.episode_title),
            )
            .await?;
        }
        None => {
            println!("\nResume canceled.");
        }
    }

    Ok(())
}

fn play_video(url: &str) -> Result<Option<std::process::Child>, Box<dyn Error>> {
    let is_termux = env::var("TERMUX_VERSION").is_ok();

    if is_termux {
        let mut player = Command::new("termux-open");
        player.arg(url);
        player.spawn()?;
        Ok(None)
    } else {
        let mut player = Command::new("mpv");
        player.arg("--save-position-on-quit");
        player.arg(url);
        player.stdout(Stdio::null());
        player.stderr(Stdio::null());

        let child = player.spawn()?;
        Ok(Some(child))
    }
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
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(fzf_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selection.is_empty() {
        return Ok(None);
    }

    if let Some(selected_id) = selection.split(" | ").next() {
        let clean_id = selected_id.trim();
        if let Some(selected_show) = shows.iter().find(|s| s.id == clean_id) {
            return Ok(Some(selected_show.clone()));
        }
    }

    Ok(None)
}

async fn scrape_episodes(play_id: &str) -> Result<Vec<Episode>, Box<dyn Error>> {
    let url = format!("http://10.16.100.244/player.php?play={}", play_id);
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
        .spawn()?;

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

fn post_play_menu(current_idx: usize, total_eps: usize) -> Result<PostPlayAction, Box<dyn Error>> {
    let mut options = Vec::new();

    if current_idx + 1 < total_eps {
        options.push("Next Episode");
    }
    if current_idx > 0 {
        options.push("Previous Episode");
    }

    options.push("Replay");
    options.push("Select Another Episode");
    options.push("Quit");

    let fzf_input = options.join("\n") + "\n";

    let mut child = Command::new("fzf")
        .arg("--prompt=▶ Playing: ")
        .arg("--layout=reverse")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(fzf_input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();

    match selection.as_str() {
        "Next Episode" => Ok(PostPlayAction::Next),
        "Previous Episode" => Ok(PostPlayAction::Prev),
        "Replay" => Ok(PostPlayAction::Replay),
        "Select Another Episode" => Ok(PostPlayAction::Episodes),
        _ => Ok(PostPlayAction::Quit),
    }
}

async fn handle_show(
    show_id: &str,
    show_name: &str,
    initial_episode_title: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let episodes = scrape_episodes(show_id).await?;

    if episodes.is_empty() {
        println!("No video links found.");
        return Ok(());
    }

    let mut current_idx = match initial_episode_title {
        Some(title) => episodes.iter().position(|e| e.title == title).unwrap_or(0),
        None => match select_episode_with_fzf(&episodes)? {
            Some(ep) => episodes.iter().position(|e| e.title == ep.title).unwrap(),
            None => return Ok(()),
        },
    };

    loop {
        let current_ep = &episodes[current_idx];

        let current_watch = HistoryItem {
            show_id: show_id.to_string(),
            show_name: show_name.to_string(),
            episode_title: current_ep.title.clone(),
            url: current_ep.url.clone(),
        };

        let _ = save_history(current_watch);

        let child_opt = match play_video(&current_ep.url) {
            Ok(child) => child,
            Err(_) => break,
        };

        let action = post_play_menu(current_idx, episodes.len())?;

        if let Some(mut child) = child_opt {
            let _ = child.kill();
            let _ = child.wait();
        }

        match action {
            PostPlayAction::Next => current_idx += 1,
            PostPlayAction::Prev => current_idx -= 1,
            PostPlayAction::Replay => continue,
            PostPlayAction::Episodes => match select_episode_with_fzf(&episodes)? {
                Some(ep) => {
                    current_idx = episodes.iter().position(|e| e.title == ep.title).unwrap()
                }
                None => break,
            },
            PostPlayAction::Quit => break,
        }
    }

    Ok(())
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
                handle_show(&selected_show.id, &selected_show.name, None).await?;
            }
            None => println!("\nNo show selected."),
        }
    } else {
        eprintln!("Status code: {}", fetch.status());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let input: String;

    if args.len() < 2 {
        print!("\x1b[1;36mEnter search query: \x1b[0m");
        io::stdout().flush().unwrap();

        let mut user_input = String::new();
        io::stdin()
            .read_line(&mut user_input)
            .expect("Failed to read line");
        input = user_input.trim().to_string();

        if input.is_empty() {
            eprintln!("\x1b[31mNo query provided. Exiting.\x1b[0m");
            std::process::exit(1);
        }
    } else {
        input = args[1].clone();
    }
    if input == "-c" {
        if let Err(e) = continue_watching().await {
            eprintln!("Application Error: {}", e);
        }
    } else {
        if let Err(e) = fetch_shows(&input).await {
            eprintln!("Application Error: {}", e);
        }
    }
}
