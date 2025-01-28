use crate::database::Database;
use crate::knowledge_base::knowledge_base::KnowledgeBaseHandler;
use log::info;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Insight {
    pub topic: String,
    pub context: String,
    pub confidence: f32,
    pub source: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningContext {
    pub insights: Vec<Insight>,
    pub related_topics: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl LearningContext {
    pub fn new() -> Self {
        Self {
            insights: Vec::new(),
            related_topics: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn extract_from_interaction(user_input: &str, ai_response: &str) -> Self {
        let mut context = Self::new();
        let now = chrono::Utc::now();

        // Extract topics and insights from user input
        let topics = Self::extract_topics(user_input);
        for topic in &topics {
            context.related_topics.push(topic.clone());
            context.insights.push(Insight {
                topic: topic.clone(),
                context: user_input.to_string(),
                confidence: 0.7,
                source: "user_input".to_string(),
                timestamp: now,
            });
        }

        // Extract insights from AI response
        let response_insights = Self::extract_insights(ai_response);
        for (topic, content) in response_insights {
            context.insights.push(Insight {
                topic,
                context: content,
                confidence: 0.8,
                source: "ai_response".to_string(),
                timestamp: now,
            });
        }

        context
    }

    fn extract_topics(text: &str) -> Vec<String> {
        let mut topics = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();
        
        // Simple topic extraction (can be enhanced with NLP)
        for window in words.windows(2) {
            if window[0].len() > 3 && window[1].len() > 3 {
                topics.push(format!("{} {}", window[0], window[1]));
            }
        }

        topics
    }

    fn extract_insights(text: &str) -> Vec<(String, String)> {
        let mut insights = Vec::new();
        let sentences: Vec<&str> = text.split(['.', '!', '?']).collect();

        for sentence in sentences {
            let sentence = sentence.trim();
            if sentence.len() > 20 {
                if let Some(topic) = Self::identify_main_topic(sentence) {
                    insights.push((topic, sentence.to_string()));
                }
            }
        }

        insights
    }

    fn identify_main_topic(sentence: &str) -> Option<String> {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        words.first().map(|w| w.to_string())
    }
}

pub struct LearningManager {
    db: Arc<Database>,
    knowledge_base: Arc<KnowledgeBaseHandler>,
    context_cache: Arc<Mutex<HashMap<String, LearningContext>>>,
}

impl LearningManager {
    pub fn new(db: Database, knowledge_base: KnowledgeBaseHandler) -> Self {
        Self {
            db: Arc::new(db),
            knowledge_base: Arc::new(knowledge_base),
            context_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn learn_from_interaction(
        &self,
        user_input: &str,
        ai_response: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Extract learning context
        let context = LearningContext::extract_from_interaction(user_input, ai_response);
        
        // Store insights in database
        for insight in &context.insights {
            self.db.save_knowledge(
                format!("insight:{}:{}", insight.topic, insight.timestamp.timestamp()),
                serde_json::to_string(&insight)?,
            ).await?;
        }

        // Update knowledge base
        for topic in &context.related_topics {
            if let Some(existing) = self.knowledge_base.get_entry(topic).await? {
                let mut entry = existing;
                entry.push_str("\n");
                entry.push_str(ai_response);
                self.knowledge_base.update_entry(topic, &entry).await?;
            } else {
                self.knowledge_base.add_entry(topic, ai_response).await?;
            }
        }

        // Update context cache
        let mut cache = self.context_cache.lock().await;
        for topic in &context.related_topics {
            cache.insert(topic.clone(), context.clone());
        }

        info!("Learned from interaction: {} insights, {} topics", 
            context.insights.len(), 
            context.related_topics.len()
        );

        Ok(())
    }

    pub async fn get_relevant_context(
        &self,
        query: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut context = Vec::new();

        // Check context cache first
        let cache = self.context_cache.lock().await;
        for topic in LearningContext::extract_topics(query) {
            if let Some(cached_context) = cache.get(&topic) {
                for insight in &cached_context.insights {
                    context.push(insight.context.clone());
                }
            }
        }

        // Get insights from database
        let topics = LearningContext::extract_topics(query);
        for topic in &topics {
            if let Some(value) = self.db.get_knowledge(format!("topic:{}", topic)).await? {
                if let Ok(insight) = serde_json::from_str::<Insight>(&value) {
                    context.push(insight.context);
                }
            }
        }

        // Get knowledge base entries
        for topic in &topics {
            if let Some(entry) = self.knowledge_base.get_entry(topic).await? {
                context.push(entry);
            }
        }

        Ok(context)
    }

    pub async fn get_learning_summary(&self) -> Result<String, Box<dyn std::error::Error>> {
        let cache = self.context_cache.lock().await;
        let mut summary = String::new();

        summary.push_str("Learning Summary:\n\n");

        // Summarize topics
        let mut all_topics = Vec::new();
        for context in cache.values() {
            all_topics.extend(context.related_topics.clone());
        }
        all_topics.sort();
        all_topics.dedup();

        summary.push_str(&format!("Topics Learned: {}\n", all_topics.join(", ")));

        // Summarize insights
        let mut total_insights = 0;
        for context in cache.values() {
            total_insights += context.insights.len();
        }
        summary.push_str(&format!("\nTotal Insights: {}\n", total_insights));

        Ok(summary)
    }
}
