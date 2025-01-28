use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use anyhow::{Result, Error as AnyhowError};
use colored::Colorize;
use std::fs::{OpenOptions, File};
use std::io::{Write, BufRead, BufReader};

use crate::personality::PersonalityProfile;
use crate::providers::twitter::twitbrain::{TwitterProvider, TweetStatus, Mention};
use crate::providers::twitter::composer::TweetComposer;

// Constants
const DEFAULT_EMOJI: &str = "💭";

pub struct ConversationManager {
    profile: Arc<RwLock<PersonalityProfile>>,
    twitter: Arc<TwitterProvider>,
    auto_post_enabled: Arc<AtomicBool>,
    auto_post_task: Option<JoinHandle<()>>,
}

impl ConversationManager {
    pub async fn new(profile: PersonalityProfile) -> Result<Self> {
        let twitter = TwitterProvider::new().await
            .map_err(|e| AnyhowError::msg(e.to_string()))?;
        
        Ok(Self { 
            profile: Arc::new(RwLock::new(profile)),
            twitter,
            auto_post_enabled: Arc::new(AtomicBool::new(false)),
            auto_post_task: None,
        })
    }

    pub async fn update_personality(&mut self, profile: PersonalityProfile) {
        let mut current_profile = self.profile.write().await;
        *current_profile = profile;
    }

    async fn log_twitter_activity(&self, message: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open("/var/log/twitter/twitter.log")?;
            
        let profile = self.profile.read().await;
        writeln!(file, "[{}] [{}] {}", 
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            profile.name,
            message)
    }

    pub async fn generate_and_post_tweet(&self) -> Result<String> {
        // Verify and log the current profile
        let profile_guard = self.profile.read().await;
        let profile = &*profile_guard;
        let name = profile.name.clone();
        let desc = profile.get_str("description").unwrap_or_default();
        
        println!("🤖 Generating tweet as: {}", name);
        println!("Character: {}", desc);
        
        // First generate a topic using the verified profile
        let topic = TweetComposer::generate_auto_post_topic(profile).await?;
        println!("📝 Generated topic: \"{}\"", topic);
        
        // Then generate a tweet about that topic using the same profile
        let tweet = TweetComposer::generate_auto_tweet(profile).await?;
        println!("✍️ Generated tweet in {}'s style", name);
        Ok(tweet)
    }

    pub async fn handle_command(&mut self, input: &str) -> Result<()> {
        // Show token count for input
        let token_count = input.split_whitespace().count();
        println!("📊 Input tokens: {}", token_count);

        match input.trim() {
            "tweet" => {
                println!("🤖 Generating AI tweet...");
                match self.generate_and_post_tweet().await {
                    Ok(tweet_content) => {
                        println!("📝 Generated tweet: \"{}\"", tweet_content);
                        println!("\nWould you like to post this tweet? (y/n)");
                        
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        
                        if input.trim().to_lowercase() == "y" {
                            match self.direct_tweet(&tweet_content).await {
                                Ok(status) => {
                                    println!("✅ Tweet posted successfully!");
                                    println!("🔗 Tweet URL: {}", status.url);
                                },
                                Err(e) => println!("❌ Failed to post tweet: {}", e)
                            }
                        } else {
                            println!("Tweet cancelled.");
                        }
                    },
                    Err(e) => println!("❌ Failed to generate AI tweet: {}", e)
                }
            },
            
            s if s.starts_with("tweet ") => {
                let content = s.trim_start_matches("tweet ").trim();
                if content.is_empty() {
                    println!("❌ Tweet content cannot be empty");
                    return Ok(());
                }

                println!("🐦 Posting tweet: \"{}\"", content);
                match self.direct_tweet(content).await {
                    Ok(status) => {
                        println!("✅ Tweet posted successfully!");
                        println!("🔗 Tweet URL: {}", status.url);
                    },
                    Err(e) => println!("❌ Failed to post tweet: {}", e)
                }
            },

            s if s.starts_with("autopost start ") => {
                let minutes = s.trim_start_matches("autopost start ").trim();
                if let Ok(mins) = minutes.parse::<u64>() {
                    println!("🤖 Starting auto-post every {} minutes...", mins);
                    println!("(Type 'autopost stop' to stop auto-posting)");
                    
                    let auto_post_enabled = self.auto_post_enabled.clone();
                    let profile = self.profile.clone();
                    let twitter = self.twitter.clone();

                    let task = tokio::spawn(async move {
                        while auto_post_enabled.load(Ordering::SeqCst) {
                            // Get the current profile
                            let profile_guard = profile.read().await;
                            let current_profile = &*profile_guard;
                            match TweetComposer::generate_auto_tweet(current_profile).await {
                                Ok(tweet_content) => {
                                    match twitter.post_tweet(&tweet_content, true).await {
                                        Ok(status) => {
                                            println!("✅ Auto-tweet posted successfully as {}!", current_profile.name);
                                            println!("🔗 Tweet URL: {}", status.url);
                                        },
                                        Err(e) => println!("❌ Failed to post tweet: {}", e)
                                    }
                                },
                                Err(e) => println!("❌ Failed to generate tweet: {}", e)
                            }
                            // Drop the read lock
                            drop(profile_guard);

                            println!("⏰ Next auto-tweet in {} minutes...", mins);
                            tokio::time::sleep(tokio::time::Duration::from_secs(mins * 60)).await;
                        }
                        println!("Auto-posting stopped.");
                    });

                    self.auto_post_task = Some(task);
                    self.auto_post_enabled.store(true, Ordering::SeqCst);
                    println!("Auto-posting is running in the background. You can continue chatting!");
                } else {
                    println!("❌ Invalid minutes value. Please use a number.");
                    println!("Example: autopost start 30");
                }
            },

            "autopost stop" => {
                self.auto_post_enabled.store(false, Ordering::SeqCst);
                println!("🛑 Stopping auto-post...");
                if let Some(task) = self.auto_post_task.take() {
                    task.abort();
                    println!("Auto-posting stopped successfully!");
                } else {
                    println!("No auto-posting task was running.");
                }
            },

            s if s.starts_with("reply ") => {
                if let Some((tweet_id, content)) = s.trim_start_matches("reply ").split_once(' ') {
                    println!("🔄 Posting reply to tweet {}...", tweet_id);
                    match self.twitter.reply_to_tweet(tweet_id.trim(), content.trim()).await {
                        Ok(status) => {
                            println!("✅ Reply posted successfully!");
                            println!("🔗 Reply URL: {}", status.url);
                        },
                        Err(e) => println!("❌ Failed to post reply: {}", e)
                    }
                } else {
                    println!("❌ Invalid reply format. Use: reply <tweet_id> <your reply>");
                }
            },

            s if s.starts_with("dm @") => {
                if let Some((username, message)) = s.trim_start_matches("dm @").split_once(": ") {
                    println!("📨 Sending DM to @{}...", username);
                    match self.twitter.send_dm(username.trim(), message.trim()).await {
                        Ok(_) => println!("✅ DM sent successfully!"),
                        Err(e) => println!("❌ Failed to send DM: {}", e)
                    }
                } else {
                    println!("❌ Invalid DM format. Use: dm @username: your message");
                }
            },

            "logs" | "log" => {
                println!("📋 Recent Twitter Activity:");
                println!("{}", "─".repeat(50).bright_black());
                match self.twitter.get_logs(10) {
                    Ok(logs) => {
                        for log in logs {
                            println!("  {}", log);
                        }
                        println!("{}", "─".repeat(50).bright_black());
                    },
                    Err(e) => println!("❌ Error reading logs: {}", e)
                }
            },

            s if s.starts_with("logs ") => {
                if let Ok(count) = s.trim_start_matches("logs ").trim().parse::<usize>() {
                    println!("📋 Last {} Twitter Activities:", count);
                    println!("{}", "─".repeat(50).bright_black());
                    match self.twitter.get_logs(count) {
                        Ok(logs) => {
                            for log in logs {
                                println!("  {}", log);
                            }
                            println!("{}", "─".repeat(50).bright_black());
                        },
                        Err(e) => println!("❌ Error reading logs: {}", e)
                    }
                } else {
                    println!("❌ Invalid number. Usage: logs <number>");
                    println!("Example: logs 20");
                }
            },

            s if s.starts_with("autoreply ") => {
                if let Some((tweet_id, tweet_text)) = s.trim_start_matches("autoreply ").split_once(' ') {
                    println!("🤖 Generating AI reply to tweet: \"{}\"", tweet_text);
                    let profile_guard = self.profile.read().await;
                    let profile = &*profile_guard;
                    match TweetComposer::generate_auto_reply(profile, tweet_text).await {
                        Ok(reply) => {
                            println!("📝 Generated reply: \"{}\"", reply);
                            println!("\nWould you like to post this reply? (y/n)");
                            
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input)?;
                            
                            if input.trim().to_lowercase() == "y" {
                                match self.twitter.reply_to_tweet(tweet_id.trim(), &reply).await {
                                    Ok(status) => {
                                        println!("✅ Reply posted successfully!");
                                        println!("🔗 Reply URL: {}", status.url);
                                    },
                                    Err(e) => println!("❌ Failed to post reply: {}", e)
                                }
                            } else {
                                println!("Reply cancelled.");
                            }
                        },
                        Err(e) => println!("❌ Failed to generate AI reply: {}", e)
                    }
                } else {
                    println!("❌ Invalid autoreply format. Use: autoreply <tweet_id> <original tweet text>");
                }
            },

            s if s.starts_with("autodm @") => {
                if let Some((username, _)) = s.trim_start_matches("autodm @").split_once(": ") {
                    println!("🤖 Generating AI DM for @{}...", username);
                    let profile_guard = self.profile.read().await;
                    let profile = &*profile_guard;
                    match TweetComposer::generate_dm(profile, username).await {
                        Ok(dm) => {
                            println!("📝 Generated DM: \"{}\"", dm);
                            println!("\nWould you like to send this DM? (y/n)");
                            
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input)?;
                            
                            if input.trim().to_lowercase() == "y" {
                                match self.twitter.send_dm(username.trim(), &dm).await {
                                    Ok(_) => println!("✅ DM sent successfully!"),
                                    Err(e) => println!("❌ Failed to send DM: {}", e)
                                }
                            } else {
                                println!("DM cancelled.");
                            }
                        },
                        Err(e) => println!("❌ Failed to generate AI DM: {}", e)
                    }
                } else {
                    println!("❌ Invalid autodm format. Use: autodm @username: any context");
                }
            },

            s if s.starts_with("automention ") => {
                if let Some((username, mention_text)) = s.trim_start_matches("automention ").split_once(' ') {
                    println!("🤖 Generating AI response to mention from @{}...", username);
                    let profile_guard = self.profile.read().await;
                    let profile = &*profile_guard;
                    let mention = Mention {
                        id: None,
                        text: mention_text.to_string()
                    };
                    match TweetComposer::generate_mention_response(profile, &mention).await {
                        Ok(response) => {
                            println!("📝 Generated response: \"{}\"", response);
                            println!("\nWould you like to post this response? (y/n)");
                            
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input)?;
                            
                            if input.trim().to_lowercase() == "y" {
                                match self.twitter.post_tweet(&response, false).await {
                                    Ok(status) => {
                                        println!("✅ Response posted successfully!");
                                        println!("🔗 Response URL: {}", status.url);
                                    },
                                    Err(e) => println!("❌ Failed to post response: {}", e)
                                }
                            } else {
                                println!("Response cancelled.");
                            }
                        },
                        Err(e) => println!("❌ Failed to generate AI response: {}", e)
                    }
                } else {
                    println!("❌ Invalid automention format. Use: automention @username mention text");
                }
            },

            "topic" => {
                println!("🤖 Generating tweet topic...");
                let profile_guard = self.profile.read().await;
                let profile = &*profile_guard;
                match TweetComposer::generate_auto_post_topic(profile).await {
                    Ok(topic) => {
                        println!("📝 Generated topic: \"{}\"", topic);
                        println!("\nWould you like to generate a tweet about this topic? (y/n)");
                        
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        
                        if input.trim().to_lowercase() == "y" {
                            match TweetComposer::generate_auto_tweet(profile).await {
                                Ok(tweet_content) => {
                                    println!("📝 Generated tweet: \"{}\"", tweet_content);
                                    println!("\nWould you like to post this tweet? (y/n)");
                                    
                                    let mut input = String::new();
                                    std::io::stdin().read_line(&mut input)?;
                                    
                                    if input.trim().to_lowercase() == "y" {
                                        match self.direct_tweet(&tweet_content).await {
                                            Ok(status) => {
                                                println!("✅ Tweet posted successfully!");
                                                println!("🔗 Tweet URL: {}", status.url);
                                            },
                                            Err(e) => println!("❌ Failed to post tweet: {}", e)
                                        }
                                    } else {
                                        println!("Tweet cancelled.");
                                    }
                                },
                                Err(e) => println!("❌ Failed to generate tweet: {}", e)
                            }
                        }
                    },
                    Err(e) => println!("❌ Failed to generate topic: {}", e)
                }
            },

            _ => {
                println!("Available Twitter commands:");
                println!("  tweet                     - Generate and post an AI tweet");
                println!("  tweet <message>           - Post a specific tweet");
                println!("  topic                     - Generate a tweet topic");
                println!("  autoreply <id> <text>     - Generate AI reply to a tweet");
                println!("  autodm @user: <context>   - Generate AI DM to a user");
                println!("  automention @user <text>  - Generate AI response to mention");
                println!("  autopost start <minutes>  - Start auto-posting every N minutes");
                println!("  autopost stop             - Stop auto-posting");
                println!("  reply <id> <message>      - Reply to a tweet");
                println!("  dm @user: <message>       - Send a direct message");
                println!("  logs                      - Show last 10 activities");
                println!("  logs <number>             - Show last N activities");
            }
        }
        Ok(())
    }

    fn show_logs(&self, count: usize) -> std::io::Result<()> {
        let log_path = "/var/log/twitter/twitter.log";
        let file = match File::open(log_path) {
            Ok(file) => file,
            Err(_) => {
                println!("No log file found. Start tweeting to create logs!");
                return Ok(());
            }
        };

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines()
            .filter_map(Result::ok)
            .collect();

        // Show last N entries, most recent first
        for line in lines.iter().rev().take(count) {
            println!("  {}", line);
        }
        println!("{}", "─".repeat(50).bright_black());
        Ok(())
    }

    pub async fn direct_tweet(&self, content: &str) -> Result<TweetStatus> {
        self.twitter.post_tweet(content, true).await
            .map_err(|e| AnyhowError::msg(e.to_string()))
    }

    async fn reply_to_tweet(&self, tweet_id: &str, content: &str) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.twitter.reply_to_tweet(tweet_id, content).await
    }

    async fn send_dm(&self, username: &str, content: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.twitter.send_dm(username, content).await
    }
}