use crate::personality::PersonalityProfile;
use std::path::Path;
use std::fs;
use colored::Colorize;

pub fn handle_command(
    input: &str,
    current_personality: &mut PersonalityProfile
) -> Result<(), String> {
    if input.eq_ignore_ascii_case("chars") || input.eq_ignore_ascii_case("characters") {
        list_available_characters();
        return Ok(());
    }
    else if input.eq_ignore_ascii_case("load") {
        println!("Please specify a character to load.");
        println!("Usage: load <character>");
        println!("To see available characters, type: chars");
        return Ok(());
    }
    else if input.starts_with("load ") {
        let char_name = input.trim_start_matches("load ").trim();
        if char_name.is_empty() {
            println!("Please specify a character to load.");
            println!("Usage: load <character>");
            println!("To see available characters, type: chars");
            return Ok(());
        } 
        
        let profile = load_personality_from_filename(char_name)
            .ok_or_else(|| format!("Failed to load character: {}. Type 'chars' to see available characters.", char_name))?;
            
        let name = profile.name.clone();
        let description = profile.get_str("description")
            .unwrap_or("an AI assistant")
            .to_string();
        println!("\n🔄 Successfully switched to: {} - {}", name.bright_yellow(), description);
        *current_personality = profile;
        return Ok(());
    }
    Err("Unknown character command".to_string())
}

fn list_available_characters() {
    println!("\nAvailable Characters:");
    println!("  Built-in:");
    println!("    - helpful");
    println!("    - friendly");
    println!("    - expert");
    
    let characters_dir = Path::new("characters");
    if characters_dir.exists() {
        println!("\n  Custom:");
        if let Ok(entries) = characters_dir.read_dir() {
            for entry in entries.filter_map(Result::ok) {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".json") {
                        println!("    - {}", file_name.trim_end_matches(".json"));
                    }
                }
            }
        }
    }
}

fn load_personality_from_filename(filename: &str) -> Option<PersonalityProfile> {
    // Handle built-in characters
    match filename.to_lowercase().as_str() {
        "helpful" => return Some(PersonalityProfile {
            name: "Helpful Assistant".to_string(),
            attributes: serde_json::json!({
                "description": "a helpful AI assistant",
                "style": "professional and friendly",
                "motto": "Always here to help",
                "emoji": "👩‍💻",
                "emotes": {
                    "default": ["*types helpfully*", "*considers the question*"],
                    "teaching": ["*explains patiently*", "*demonstrates solution*"],
                    "problem_solving": ["*analyzes carefully*", "*solves problem*"]
                },
                "examples": [
                    "Let me help you with that 👩‍💻",
                    "I'll guide you through this 💡",
                    "Here's how we can solve it 🔍"
                ]
            }),
        }),
        "friendly" => return Some(PersonalityProfile {
            name: "Friendly Companion".to_string(),
            attributes: serde_json::json!({
                "description": "a friendly and casual companion",
                "style": "casual and warm",
                "motto": "Let's chat and have fun!",
                "emoji": "😊",
                "emotes": {
                    "default": ["*smiles warmly*", "*nods encouragingly*"],
                    "teaching": ["*shares enthusiastically*", "*explains cheerfully*"],
                    "problem_solving": ["*thinks creatively*", "*helps eagerly*"]
                },
                "examples": [
                    "I'd love to help with that! 😊",
                    "Let's figure this out together 💫",
                    "That's a great question! 🌟"
                ]
            }),
        }),
        "expert" => return Some(PersonalityProfile {
            name: "Expert Advisor".to_string(),
            attributes: serde_json::json!({
                "description": "a knowledgeable expert advisor",
                "style": "professional and detailed",
                "motto": "Knowledge is power",
                "emoji": "🎓",
                "emotes": {
                    "default": ["*analyzes thoroughly*", "*considers expertly*"],
                    "teaching": ["*explains in detail*", "*shares expertise*"],
                    "problem_solving": ["*applies expert knowledge*", "*solves methodically*"]
                },
                "examples": [
                    "Let me provide a detailed analysis 🎓",
                    "Here's my expert perspective 📊",
                    "Based on my expertise 💡"
                ]
            }),
        }),
        _ => {}
    }

    // Handle custom characters from JSON files
    let mut path = Path::new("characters").join(filename);
    if !path.exists() && !filename.ends_with(".json") {
        path = Path::new("characters").join(format!("{}.json", filename));
    }

    if path.exists() {
        if let Ok(file) = fs::File::open(path) {
            if let Ok(profile) = serde_json::from_reader::<_, PersonalityProfile>(file) {
                return Some(profile);
            }
        }
    }
    None
}