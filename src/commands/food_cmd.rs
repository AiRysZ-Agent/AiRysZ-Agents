use crate::food::analysis::nutrition::analyze_nutrition;
use crate::food::api::spoonacular::SpoonacularClient;
use crate::food::config::FoodConfig;
use crate::providers::traits::CompletionProvider;

pub async fn handle_command(input: &str, provider: &Box<dyn CompletionProvider + Send + Sync>) -> Result<(), String> {
    let input = input.trim();
    
    let response = match input.split_whitespace().next() {
        Some("nutrition") => {
            let food_item = input.trim_start_matches("nutrition").trim();
            if food_item.is_empty() {
                return Ok(println!("Please specify a food item to analyze."));
            }
            let result = analyze_nutrition(food_item).await?;
            println!("{}", result);
            Ok::<(), String>(())
        }
        Some("recipe") => {
            let recipe_name = input.trim_start_matches("recipe").trim();
            if recipe_name.is_empty() {
                return Ok(println!("Please specify a recipe name to search."));
            }
            
            // Initialize Spoonacular client
            let config = FoodConfig::from_env()?;
            let spoonacular = SpoonacularClient::new(config.spoonacular_api_key);
            
            // Get recipe details
            let recipe_info = spoonacular.search_recipe(recipe_name).await?;
            
            if recipe_info.starts_with("No recipe found") {
                println!("❌ Recipe not found. Try:\n1. Check your spelling\n2. Use a more common name (e.g., 'pasta carbonara' instead of 'spaghetti carbonara')\n3. Simplify the search (e.g., 'carbonara' instead of 'authentic Italian carbonara')");
                return Ok::<(), String>(());
            }
            
            // Use LLM to enhance recipe information with cooking tips
            let prompt = format!(
                "Analyze this recipe with your own unique character, personality and style. Share your thoughts about:\n\n{}\n\n
                1. 🤔 Your Thoughts:\n- Initial impressions\n- What excites you about this recipe\n- How would you make it special\n\n
                2. 📝 Detailed Analysis:\n- Ingredients and techniques through your perspective and show your own emoji too\n- Your personal tips and tricks and insightfull\n- Common mistakes to avoid (in your style)\n\n
                3. 💫 Special Touch:\n- How you would serve and present it\n- Your signature modifications\n- Stay true to your own character and speak in your natural style!,
                4  quick summarize all of this with your own unique style and personality",
                recipe_info
            );
            
            let output = match provider.complete(&prompt).await {
                Ok(cooking_tips) => {
                    format!("🔍 Recipe Information:\n{}\n\n👨‍🍳 Cooking Analysis:\n{}", recipe_info, cooking_tips)
                }
                Err(_) => recipe_info // Fallback to just recipe info if LLM fails
            };
            println!("{}", output);
            Ok::<(), String>(())
        }
        _ => {
            println!("Available commands:\n- nutrition <food_item> (Get nutrition facts)\n- recipe <name> (Get detailed recipe with cooking tips)");
            Ok::<(), String>(())
        }
    }?;
    Ok::<(), String>(())
}
