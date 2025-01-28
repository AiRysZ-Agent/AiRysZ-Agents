use crate::food::api::usda::UsdaClient;
use crate::food::api::spoonacular::SpoonacularClient;
use crate::food::config::FoodConfig;

pub async fn analyze_nutrition(food_item: &str) -> Result<String, String> {
    println!("Analyzing nutrition for: {}\n", food_item);

    // Initialize clients with API keys from environment
    let config = FoodConfig::from_env()?;
    let usda_client = UsdaClient::new(config.clone());
    let spoonacular_client = SpoonacularClient::new(config.spoonacular_api_key);
    
    // Try Spoonacular first for recipe data
    match spoonacular_client.search_recipe(food_item).await {
        Ok(recipe_info) => {
            if !recipe_info.contains("No recipe found") {
                return Ok(recipe_info);
            }
        }
        Err(_) => {} // Fall back to USDA if Spoonacular fails
    }

    // Fall back to USDA for basic ingredient data
    usda_client.search_food(food_item).await
}
