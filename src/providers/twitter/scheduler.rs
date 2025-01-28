use crate::providers::twitter::twitbrain::{TwitterProvider, TweetStatus};
use std::time::Duration;

pub struct Scheduler {
    twitter: TwitterProvider
}

impl Scheduler {
    pub fn new(twitter: TwitterProvider) -> Self {
        Self { twitter }
    }

    pub async fn schedule_tweet(&self, message: &str, delay: Duration) -> Result<TweetStatus, Box<dyn std::error::Error + Send + Sync>> {
        tokio::time::sleep(delay).await;
        Ok(self.twitter.post_tweet(message, true).await?)
    }
}

