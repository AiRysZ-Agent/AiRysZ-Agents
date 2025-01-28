use super::WebCrawler;
use crate::personality::PersonalityProfile;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct WebCrawlerManager {
    crawler: Arc<Mutex<WebCrawler>>,
    profile: PersonalityProfile,

}

impl WebCrawlerManager {
    pub async fn new(profile: PersonalityProfile) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let crawler = WebCrawler::new()?;
        Ok(Self {
            crawler: Arc::new(Mutex::new(crawler)),
            profile,
        })
    }

    pub async fn analyze_url(&self, url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let crawler = self.crawler.lock().await;
        let page = crawler.visit_page(url).await?;
        Ok(page.text)
    }

    pub async fn research_topic(&self, topic: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let crawler = self.crawler.lock().await;
        let search_results = crawler.search(topic).await?;
        
        let mut findings = Vec::new();
        for url in search_results {
            if let Ok(page) = crawler.visit_page(&url).await {
                findings.push(page.text);
            }
        }
        Ok(findings)
    }

    pub async fn extract_links(&self, url: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let crawler = self.crawler.lock().await;
        let page = crawler.visit_page(url).await?;
        Ok(page.links)
    }
}
