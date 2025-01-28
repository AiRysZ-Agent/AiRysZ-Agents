use std::sync::Arc;
use anyhow::Result;
use crate::providers::twitter::twitbrain::{TwitterProvider, TweetStatus};

pub struct Conversation {
    twitter: Arc<TwitterProvider> // Use Arc<TwitterProvider>
}

impl Conversation {
    pub fn new(twitter: Arc<TwitterProvider>) -> Self {
        Self { twitter }
    }

    pub async fn reply_to_tweet(&self, tweet_id: &str, message: &str) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.twitter.reply_to_tweet(tweet_id, message).await?)
    }
}