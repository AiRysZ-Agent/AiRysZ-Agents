use crate::personality::PersonalityProfile;
use crate::providers::twitter::twitbrain::Mention;
use crate::providers::deepseek::deepseek::DeepSeekProvider;
use crate::providers::mistral::mistral::MistralProvider;
use crate::providers::openrouter::openrouter::OpenRouterProvider;
use crate::providers::openai::openai::OpenAIProvider;
use crate::providers::gemini::gemini::GeminiProvider;
use crate::providers::traits::CompletionProvider;
use crate::providers::traits::CompletionProvider as ProviderTrait;
use anyhow::{Result, Error};
use std::collections::HashSet;
use std::sync::Mutex;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};
use std::env;
use std::error::Error as StdError;
use std::sync::Arc;

const MAX_TWEET_LENGTH: usize = 270;
const DEFAULT_EMOJI: &str = "💭";
const MAX_CACHE_SIZE: usize = 1000; // Maximum number of topics to remember

lazy_static! {
    static ref TOPIC_CACHE: Mutex<Vec<(String, DateTime<Utc>)>> = Mutex::new(Vec::new());
}

pub struct TweetComposer;

#[derive(Debug)]
enum TweetProvider {
    DeepSeek,
    Mistral,
    OpenRouter,
    OpenAI,
    Gemini,
}

impl TweetProvider {
    fn from_env() -> Self {
        match env::var("TWEET_PROVIDER").unwrap_or_else(|_| "deepseek".to_string()).to_lowercase().as_str() {
            "mistral" => TweetProvider::Mistral,
            "openrouter" => TweetProvider::OpenRouter,
            "openai" => TweetProvider::OpenAI,
            "gemini" => TweetProvider::Gemini,
            _ => TweetProvider::DeepSeek,
        }
    }
}

impl TweetComposer {
    async fn get_provider(profile: &PersonalityProfile) -> Result<Arc<Box<dyn CompletionProvider + Send + Sync>>> {
        match TweetProvider::from_env() {
            TweetProvider::Mistral => {
                let api_key = std::env::var("MISTRAL_API_KEY")
                    .map_err(|_| Error::msg("MISTRAL_API_KEY environment variable is not set."))?;
                
                let system_message = Self::create_system_message(profile);
                let provider = MistralProvider::new(api_key, system_message).await
                    .map_err(|e| Error::msg(format!("Failed to create Mistral provider: {}", e)))?;
                Ok(Arc::new(Box::new(provider)))
            },
            TweetProvider::DeepSeek => {
                let api_key = std::env::var("DEEPSEEK_API_KEY")
                    .map_err(|_| Error::msg("DEEPSEEK_API_KEY environment variable is not set."))?;
                
                let system_message = Self::create_system_message(profile);
                let provider = DeepSeekProvider::new(api_key, system_message).await
                    .map_err(|e| Error::msg(format!("Failed to create DeepSeek provider: {}", e)))?;
                Ok(Arc::new(Box::new(provider)))
            },
            TweetProvider::OpenRouter => {
                let api_key = std::env::var("OPENROUTER_API_KEY")
                    .map_err(|_| Error::msg("OPENROUTER_API_KEY environment variable is not set."))?;
                
                let system_message = Self::create_system_message(profile);
                let provider = OpenRouterProvider::new(api_key, system_message).await
                    .map_err(|e| Error::msg(format!("Failed to create OpenRouter provider: {}", e)))?;
                Ok(Arc::new(Box::new(provider)))
            },
            TweetProvider::OpenAI => {
                let api_key = std::env::var("OPENAI_API_KEY")
                    .map_err(|_| Error::msg("OPENAI_API_KEY environment variable is not set."))?;
                
                let system_message = Self::create_system_message(profile);
                let provider = OpenAIProvider::new(api_key, system_message).await
                    .map_err(|e| Error::msg(format!("Failed to create OpenAI provider: {}", e)))?;
                Ok(Arc::new(Box::new(provider)))
            },
            TweetProvider::Gemini => {
                let api_key = std::env::var("GEMINI_API_KEY")
                    .map_err(|_| Error::msg("GEMINI_API_KEY environment variable is not set."))?;
                
                let system_message = Self::create_system_message(profile);
                let provider = GeminiProvider::new(api_key, system_message).await
                    .map_err(|e| Error::msg(format!("Failed to create Gemini provider: {}", e)))?;
                Ok(Arc::new(Box::new(provider)))
            }
        }
    }

    fn create_system_message(profile: &PersonalityProfile) -> String {
        let mut system_parts = vec![profile.generate_system_prompt()];

        system_parts.push(format!("\nWhen tweeting, you should:\n\
            - Share insights from your expertise in {}\n\
            - Maintain your unique voice and personality traits\n\
            - Keep your communication style consistent\n\
            - Draw from your specific knowledge and experience\n\
            - Stay authentic to your character\n\n\
            Remember: You are {} - {}. Always tweet in character.", 
            profile.get_str("expertise").unwrap_or("your field"),
            profile.name,
            profile.get_str("description").unwrap_or("an expert in your field")
        ));

        if let Some(examples) = profile.attributes.get("example_tweets") {
            if let Some(arr) = examples.as_array() {
                let example_list: Vec<String> = arr.iter()
                    .filter_map(|v| v.as_str())
                    .take(3)
                    .enumerate()
                    .map(|(i, t)| format!("{}. {}", i + 1, t))
                    .collect();
                if !example_list.is_empty() {
                    system_parts.push(format!("\nYour tweet style examples (maintain similar voice and approach):\n{}", 
                        example_list.join("\n")
                    ));
                }
            }
        }

        system_parts.join("\n")
    }

    // Helper function to count approximate tokens (rough estimation)
    fn count_tokens(text: &str) -> usize {
        // Rough approximation: split on whitespace and punctuation
        text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .count()
    }

    fn clean_old_topics() {
        let mut cache = TOPIC_CACHE.lock().unwrap();
        let one_day_ago = Utc::now() - chrono::Duration::days(1);
        cache.retain(|(_, timestamp)| *timestamp > one_day_ago);
        
        // If cache is still too large, remove oldest entries
        if cache.len() > MAX_CACHE_SIZE {
            cache.sort_by(|a, b| b.1.cmp(&a.1));
            cache.truncate(MAX_CACHE_SIZE);
        }
    }

    fn is_topic_unique(topic: &str) -> bool {
        let mut cache = TOPIC_CACHE.lock().unwrap();
        !cache.iter().any(|(cached_topic, _)| 
            cached_topic.to_lowercase().contains(&topic.to_lowercase()) || 
            topic.to_lowercase().contains(&cached_topic.to_lowercase())
        )
    }

    pub async fn generate_auto_post_topic(profile: &PersonalityProfile) -> Result<String> {
        // Clean old topics first
        Self::clean_old_topics();

        for attempt in 0..3 {  // Try up to 3 times to get a unique topic
            let mut prompt_parts = vec![
                format!("You are {}", profile.name),
                format!("Role: {}", profile.get_str("description").unwrap_or_default()),
                format!("Style: {}", profile.get_str("style").unwrap_or_default())
            ];
            
            // Add personality traits
            if let Some(traits) = profile.get_array("traits") {
                let trait_list: Vec<_> = traits.iter()
                    .filter_map(|v| v.as_str())
                    .collect();
                if !trait_list.is_empty() {
                    prompt_parts.push(format!("Core personality traits: {}", trait_list.join(", ")));
                }
            }

            prompt_parts.push(format!("Current time: {}", Utc::now()));

            // Add expertise and interests
            if let Some(interests) = profile.get_array("interests") {
                let interest_list: Vec<_> = interests.iter()
                    .filter_map(|v| v.as_str())
                    .collect();
                if !interest_list.is_empty() {
                    prompt_parts.push(format!("Primary areas of expertise: {}", interest_list.join(", ")));
                }
            }

            prompt_parts.push(format!("\nTask: Generate a COMPLETELY NEW and UNIQUE topic that:
1. Has never been discussed before in your previous tweets
2. Reflects your specific expertise and interests
3. Maintains your unique personality and communication style
4. Demonstrates your depth of knowledge in your field
5. Feels authentic to your character's background
6. Aligns with your typical discussion topics
7. Must be different from any previous topics
8. Should be fresh and innovative

Generate a unique topic for timestamp {}\n\nTopic:", Utc::now()));

            let prompt = prompt_parts.join("\n\n");
            
            let provider = Self::get_provider(profile).await?;
            let topic = provider.complete(&prompt).await
                .map_err(|e| Error::msg(format!("Failed to generate topic: {}", e)))?;
            
            let topic = topic.trim()
                .trim_start_matches("Topic:")
                .trim_start_matches("\"")
                .trim_end_matches("\"")
                .trim()
                .to_string();
            
            if Self::is_topic_unique(&topic) {
                let mut cache = TOPIC_CACHE.lock().unwrap();
                cache.push((topic.clone(), Utc::now()));
                return Ok(topic);
            }

            if attempt == 2 {
                let timestamped_topic = format!("{} ({})", topic, Utc::now().timestamp());
                let mut cache = TOPIC_CACHE.lock().unwrap();
                cache.push((timestamped_topic.clone(), Utc::now()));
                return Ok(timestamped_topic);
            }
        }
        
        Err(Error::msg("Failed to generate unique topic after multiple attempts"))
    }

    #[inline]
    pub async fn generate_auto_tweet(profile: &PersonalityProfile) -> Result<String> {
        let topic = Self::generate_auto_post_topic(profile).await?;
        
        let mut prompt_parts = vec![
            format!("You are {} - {}", 
                profile.name,
                profile.get_str("description").unwrap_or_default()
            )
        ];

        prompt_parts.push(profile.generate_system_prompt());

        prompt_parts.push(format!("\nTask: Write a tweet about this topic : \"{}\"\n\nRequirements:\n\
            1. Write authentically as {} - maintain your unique voice\n\
            2. Draw from your expertise in {}\n\
            3. Use your characteristic communication style\n\
            4. Keep your personality traits consistent\n\
            5. Stay within Twitter's character limit at 260 character \n\
            6. Make it engaging and true to your character\n\n\
            Tweet:", 
            topic,
            profile.name,
            profile.get_str("expertise").unwrap_or("your field")
        ));

        let prompt = prompt_parts.join("\n\n");
        let provider = Self::get_provider(profile).await?;
        let tweet = provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to generate tweet: {}", e)))?;
        
        Ok(Self::truncate_content(tweet.trim()
            .trim_start_matches("Tweet:")
            .trim_start_matches("\"")
            .trim_end_matches("\"")
            .trim()
            .to_string()))
    }

    pub async fn generate_auto_reply(profile: &PersonalityProfile, original_tweet: &str) -> Result<String> {
        let provider = Self::get_provider(profile).await?;
        let prompt = format!(
            "As {}, create a thoughtful reply to this tweet: '{}' \
             Maintain your unique voice while adding value to the conversation.",
            profile.name,
            original_tweet
        );
        let reply = provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to generate reply: {}", e)))?;
        Ok(Self::truncate_content(reply))
    }

    pub async fn generate_dm(profile: &PersonalityProfile, recipient: &str) -> Result<String> {
        let provider = Self::get_provider(profile).await?;
        let prompt = format!(
            "As {}, write a professional direct message to @{}. \
             Keep it friendly yet professional, reflecting your personality.",
            profile.name,
            recipient
        );
        let dm = provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to generate DM: {}", e)))?;
        Ok(Self::truncate_content(dm))
    }

    pub async fn generate_mention_response(profile: &PersonalityProfile, mention: &Mention) -> Result<String> {
        let provider = Self::get_provider(profile).await?;
        let prompt = format!(
            "As {}, respond to this mention: '{}' \
             Keep your response engaging and authentic to your character.",
            profile.name,
            mention.text
        );
        let response = provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to generate mention response: {}", e)))?;
        Ok(Self::truncate_content(response))
    }

    fn truncate_content(content: String) -> String {
        content.chars().take(MAX_TWEET_LENGTH).collect()
    }
}
