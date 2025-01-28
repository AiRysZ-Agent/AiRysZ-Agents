use agent_twitter_client::scraper::Scraper;
use agent_twitter_client::error::TwitterError;
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use colored::*;
use std::process::Command;
use std::io::Write;
use std::fs::{OpenOptions, File};
use std::io::{BufRead, BufReader};
use chrono::Local;
use std::error::Error;

pub fn open_twitter_monitor() -> Result<std::process::Child, std::io::Error> {
    // Create the log file if it doesn't exist
    std::fs::OpenOptions::new()  
        .create(true)
        .append(true)
        .open("/tmp/twitter_status.log")?;

    // Launch a new screen session for the monitor
    Command::new("screen")
        .args(&[
            "-dmS", 
            "twitter_monitor",
            "bash",
            "-c",
            "clear && echo -e '\\033[1;36mTwitter Status Monitor\\033[0m' && echo '================' && tail -f /tmp/twitter_status.log"
        ])
        .spawn()?;

    println!("{}", "\nTwitter Status Monitor launched in separate terminal!".green());
    println!("To view the monitor:");
    println!("1. Open a new terminal and connect to your VPS");
    println!("2. Run: {}", "screen -r twitter_monitor".cyan());
    println!("3. To detach from monitor: Press Ctrl+A then D");
    println!("4. To kill monitor: Press Ctrl+A then K\n");

    Ok(Command::new("sleep").arg("1").spawn()?)
}

pub fn log_to_twitter_monitor(message: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .open("/tmp/twitter_status.log") 
    {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(file, "[{}] {}", timestamp, message).ok();
    }
}

#[derive(Debug)]
pub struct TweetStatus {
    pub tweet_id: String,
    pub url: String
}

#[derive(Debug)]
pub enum LogType {
    Tweet,
    AutoTweet,
    Reply,
    DM,
    Error,
    Info,
    System
}

impl ToString for LogType {
    fn to_string(&self) -> String {
        match self {
            LogType::Tweet => "TWEET".to_string(),
            LogType::AutoTweet => "AUTO-TWEET".to_string(),
            LogType::Reply => "REPLY".to_string(),
            LogType::DM => "DM".to_string(),
            LogType::Error => "ERROR".to_string(),
            LogType::Info => "INFO".to_string(),
            LogType::System => "SYSTEM".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct TwitterProvider {
    scraper: Arc<Scraper>,
    log_path: String,
}

#[derive(Debug)]
pub struct Mention {
    pub id: Option<String>,
    pub text: String
}

impl TwitterProvider {
    pub async fn new() -> Result<Arc<Self>, Box<dyn Error + Send + Sync>> {
        // Ensure .env is loaded
        match dotenv() {
            Ok(_) => println!("Loaded .env file"),
            Err(e) => println!("Warning: Could not load .env file: {}", e),
        }
        
        // Debug: Print current directory and env vars
        println!("Current directory: {:?}", std::env::current_dir()?);
        println!("Checking environment variables...");
        
        // Load and verify all required environment variables
        let cookie_string = match env::var("TWITTER_COOKIE_STRING") {
            Ok(val) => {
                println!("Found TWITTER_COOKIE_STRING");
                val.replace("\"", "") // Remove quotes if present
            },
            Err(e) => {
                println!("Error loading TWITTER_COOKIE_STRING: {}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "TWITTER_COOKIE_STRING not found")));
            }
        };

        let _x_csrf_token = match env::var("TWITTER_X_CSRF_TOKEN") {
            Ok(token) => token,
            Err(_) => String::new(),
        };

        println!("Creating scraper with credentials...");
        
        // Create scraper with all required headers
        let mut scraper = Scraper::new().await?;
        
        // Set cookies from the cookie string (hiding sensitive info)
        println!("Setting cookies...");
        scraper.set_from_cookie_string(&cookie_string).await?;
        
        println!("Scraper created successfully");

        let log_path = "logs/twitter.log".to_string();
        
        // Create logs directory if it doesn't exist
        std::fs::create_dir_all("logs")?;
        
        Ok(Arc::new(Self {
            scraper: Arc::new(scraper),
            log_path,
        }))
    }

    pub fn log_activity(&self, log_type: LogType, message: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.log_path)?;
            
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_entry = format!("[{}] [{}] {}", 
            timestamp,
            log_type.to_string(),
            message
        );
            
        writeln!(file, "{}", log_entry)
    }

    pub fn get_logs(&self, count: usize) -> std::io::Result<Vec<String>> {
        let file = match File::open(&self.log_path) {
            Ok(file) => file,
            Err(_) => return Ok(vec!["No log file found. Start tweeting to create logs!".to_string()]),
        };

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines()
            .filter_map(Result::ok)
            .collect();

        Ok(lines.iter().rev().take(count).cloned().collect())
    }

    pub async fn post_tweet(&self, content: &str, is_auto: bool) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        let log_message = format!("Posting tweet: {}", content);
        self.log_activity(if is_auto { LogType::AutoTweet } else { LogType::Tweet }, &log_message)?;
        println!("Generating tweet content...");
        println!("Generated tweet: {}", content.bright_white());
        
        if !is_auto {
            println!("\nWould you like to post this tweet? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            
            if input.trim().to_lowercase() != "y" {
                println!("{}", "Tweet cancelled.".yellow());
                return Err(Box::new(TwitterError::Auth("Tweet cancelled by user".into())));
            }
        }

        println!("Sending tweet to Twitter...");
        match self.scraper.send_tweet(content, None, None).await {
            Ok(response) => {
                // Parse the response to extract the tweet ID
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response.to_string()) {
                    if let Some(tweet_id) = json["data"]["create_tweet"]["tweet_results"]["result"]["rest_id"].as_str() {
                        let url = format!("https://twitter.com/i/status/{}", tweet_id);
                        println!("Tweet successfully posted!");
                        println!("Tweet URL: {}", url);
                        return Ok(TweetStatus { tweet_id: tweet_id.to_string(), url });
                    }
                }
                // Fallback if we can't parse the ID
                println!("Warning: Could not parse tweet ID from response. Using full response as ID.");
                let id = response.to_string();
                let url = format!("https://twitter.com/i/status/{}", id);
                Ok(TweetStatus { tweet_id: id, url })
            },
            Err(e) => {
                println!("Failed to post tweet: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn post_tweet_direct(&self, tweet: &str) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.post_tweet(tweet, true).await
    }

    pub async fn reply_to_tweet(&self, tweet_id: &str, content: &str) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.log_activity(LogType::Reply, &format!("To tweet {}: {}", tweet_id, content))?;
        println!("Generated reply: {}", content.bright_white());
        
        println!("Sending reply to tweet {}...", tweet_id);
        match self.scraper.send_tweet(content, Some(tweet_id), None).await {
            Ok(response) => {
                // Parse the response to extract the tweet ID
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response.to_string()) {
                    if let Some(reply_id) = json["data"]["create_tweet"]["tweet_results"]["result"]["rest_id"].as_str() {
                        let url = format!("https://twitter.com/i/status/{}", reply_id);
                        println!("Reply successfully posted!");
                        println!("Reply URL: {}", url);
                        return Ok(TweetStatus { tweet_id: reply_id.to_string(), url });
                    }
                }
                // Fallback if we can't parse the ID
                println!("Warning: Could not parse reply ID from response. Using full response as ID.");
                let id = response.to_string();
                let url = format!("https://twitter.com/i/status/{}", id);
                Ok(TweetStatus { tweet_id: id, url })
            },
            Err(e) => {
                println!("Failed to post reply: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn reply_to_tweet_direct(&self, tweet_id: &str, reply: &str) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.reply_to_tweet(tweet_id, reply).await
    }

    pub async fn send_dm(&self, username: &str, content: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.log_activity(LogType::DM, &format!("To @{}: {}", username, content))?;
        match self.scraper.get_direct_message_conversations(username, None).await {
            Ok(conversations) => {
                let conversation_id = conversations.conversations[0].conversation_id.clone();
                match self.scraper.send_direct_message(&conversation_id, content).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Box::new(e))
                }
            },
            Err(e) => Err(Box::new(e))
        }
    }

    pub async fn get_mentions(&self, _since_id: Option<&str>) -> Result<Vec<Mention>, Box<dyn std::error::Error + Send + Sync>> {
        match self.scraper.get_home_timeline(10, vec![]).await {
            Ok(tweets) => Ok(tweets.into_iter()
                .filter(|tweet| tweet["text"].as_str()
                    .map(|t| t.starts_with("@"))
                    .unwrap_or(false))
                .map(|tweet| Mention {
                    id: Some(tweet["id_str"].as_str().unwrap_or("").to_string()),
                    text: tweet["text"].as_str().unwrap_or("").to_string()
                })
                .collect()),
            Err(e) => Err(Box::new(e))
        }
    }

    // Add error logging
    pub fn log_error(&self, error: &str) -> std::io::Result<()> {
        self.log_activity(LogType::Error, error)
    }

    // Add system logging
    pub fn log_system(&self, message: &str) -> std::io::Result<()> {
        self.log_activity(LogType::System, message)
    }

    // Add info logging
    pub fn log_info(&self, message: &str) -> std::io::Result<()> {
        self.log_activity(LogType::Info, message)
    }
}
