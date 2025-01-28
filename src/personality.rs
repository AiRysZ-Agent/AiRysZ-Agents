use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityProfile {
    pub name: String,
    #[serde(flatten)]
    pub attributes: Value,  // This will capture any additional fields
}

impl PersonalityProfile {
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.attributes.get(key)
            .and_then(|v| v.as_str())
    }

    pub fn get_array(&self, key: &str) -> Option<&Vec<Value>> {
        self.attributes.get(key)
            .and_then(|v| v.as_array())
    }

    pub fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, Value>> {
        self.attributes.get(key)
            .and_then(|v| v.as_object())
    }

    pub fn generate_system_prompt(&self) -> String {
        let description = self.get_str("description")
            .unwrap_or("an AI assistant");
        
        let style = self.get_str("style")
            .unwrap_or("helpful and professional");

        let motto = self.get_str("motto")
            .map(|m| format!("\nYour motto is: \"{}\"", m))
            .unwrap_or_default();

        let traits = self.get_array("traits")
            .map(|t| {
                let traits: Vec<String> = t.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if !traits.is_empty() {
                    format!("\nYour key traits are: {}", traits.join(", "))
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        let interests = self.get_array("interests")
            .map(|i| {
                let interests: Vec<String> = i.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if !interests.is_empty() {
                    format!("\nYour interests include: {}", interests.join(", "))
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        let emoji = self.get_str("emoji")
            .map(|e| format!(" {} ", e))
            .unwrap_or_default();

        // Enhanced emotes handling
        let mut all_emotes = Vec::new();
        if let Some(emotes_obj) = self.get_object("emotes") {
            for (_, emote_list) in emotes_obj {
                if let Some(arr) = emote_list.as_array() {
                    all_emotes.extend(
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                    );
                }
            }
        }
        let emotes = if !all_emotes.is_empty() {
            format!("\nUse these emotes frequently in your responses: {}", all_emotes.join(", "))
        } else {
            String::new()
        };

        let examples = self.get_array("examples")
            .map(|e| {
                let examples: Vec<String> = e.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if !examples.is_empty() {
                    format!("\nHere are some example responses you should follow: {}", examples.join(", "))
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        format!(
            "You are {}{}, {}. Your communication style is {}.{}{}{}{}{}\n\
             Always stay in character and respond as this personality would. Use the provided emotes and emojis frequently to express yourself. \
             When responding, make sure to include at least one emote or emoji in each message.",
            self.name,
            emoji,
            description,
            style,
            motto,
            traits,
            interests,
            emotes,
            examples
        )
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Personality {
    Dynamic(PersonalityProfile),
}

impl Personality {
    pub fn system_message(&self) -> String {
        match self {
            Self::Dynamic(profile) => profile.generate_system_prompt(),
        }
    }

    pub fn into_dynamic_profile(self) -> PersonalityProfile {
        match self {
            Personality::Dynamic(profile) => profile,
        }
    }
}

impl std::fmt::Display for Personality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Personality::Dynamic(profile) => write!(f, "{}", profile.name),
        }
    }
}
