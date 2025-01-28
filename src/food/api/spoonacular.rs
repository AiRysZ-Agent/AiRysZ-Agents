use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub id: i64,
    pub title: String,
    pub servings: Option<i32>,
    pub ready_in_minutes: Option<i32>,
    pub source_url: Option<String>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub instructions: Option<String>,
    pub extended_ingredients: Option<Vec<Ingredient>>,
    pub nutrition: Option<NutritionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ingredient {
    pub id: Option<i64>,
    pub name: String,
    pub amount: f64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NutritionInfo {
    pub nutrients: Vec<Nutrient>,
    pub calories: String,
    pub fat: String,
    pub protein: String,
    pub carbs: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Nutrient {
    pub name: String,
    pub amount: f64,
    pub unit: String,
}

#[derive(Debug)]
pub struct SpoonacularClient {
    api_key: String,
    base_url: String,
}

impl SpoonacularClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.spoonacular.com".to_string(),
        }
    }

    pub async fn search_recipe(&self, query: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let url = format!("{}/recipes/complexSearch", self.base_url);
        
        // Convert parameters to String
        let true_str = "true".to_string();
        let one_str = "1".to_string();

        // Build advanced search parameters
        let mut params = vec![
            ("apiKey", self.api_key.clone()),
            ("addRecipeInformation", true_str.clone()),
            ("addRecipeNutrition", true_str.clone()),
            ("fillIngredients", true_str.clone()),
            ("instructionsRequired", true_str),
            ("number", one_str),
        ];

        // Process query for better results
        let query_lower = query.to_lowercase();
        
        // Handle cuisine-specific searches
        if query_lower.contains("indonesian") || query_lower.contains("nasi") || query_lower.contains("mie") {
            params.push(("cuisine", "asian".to_string()));
            // If it's fried rice, expand the search
            if query_lower.contains("fried rice") || query_lower.contains("nasi goreng") {
                params.push(("query", "nasi goreng indonesian fried rice".to_string()));
            } else {
                params.push(("query", format!("indonesian {}", query)));
            }
        } else if query_lower.contains("japanese") {
            params.push(("cuisine", "japanese".to_string()));
            params.push(("query", query.to_string()));
        } else if query_lower.contains("italian") {
            params.push(("cuisine", "italian".to_string()));
            params.push(("query", query.to_string()));
        } else if query_lower.contains("indian") {
            params.push(("cuisine", "indian".to_string()));
            params.push(("query", query.to_string()));
        } else if query_lower.contains("thai") {
            params.push(("cuisine", "thai".to_string()));
            params.push(("query", query.to_string()));
        } else if query_lower.contains("chinese") {
            params.push(("cuisine", "chinese".to_string()));
            params.push(("query", query.to_string()));
        } else {
            // General search with semantic matching
            params.push(("query", query.to_string()));
        }

        // Add sorting by relevance and popularity
        params.push(("sort", "popularity".to_string()));
        params.push(("sortDirection", "desc".to_string()));

        let response = client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API request failed with status: {}", response.status()));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
            if let Some(recipe) = results.first() {
                let mut info = String::new();
                
                // Basic recipe information
                if let Some(title) = recipe.get("title").and_then(|t| t.as_str()) {
                    info.push_str(&format!("🍳 Recipe: {}\n\n", title));
                }

                // Ready time and servings
                if let (Some(time), Some(servings)) = (
                    recipe.get("readyInMinutes").and_then(|t| t.as_i64()),
                    recipe.get("servings").and_then(|s| s.as_i64())
                ) {
                    info.push_str(&format!("⏱️ Ready in: {} minutes\n👥 Servings: {}\n\n", time, servings));
                }

                // Cuisine and dish type information
                if let Some(cuisines) = recipe.get("cuisines").and_then(|c| c.as_array()) {
                    if !cuisines.is_empty() {
                        info.push_str("🌍 Cuisine: ");
                        info.push_str(&cuisines.iter()
                            .filter_map(|c| c.as_str())
                            .collect::<Vec<_>>()
                            .join(", "));
                        info.push_str("\n");
                    }
                }

                if let Some(dish_types) = recipe.get("dishTypes").and_then(|d| d.as_array()) {
                    if !dish_types.is_empty() {
                        info.push_str("🍽️ Type: ");
                        info.push_str(&dish_types.iter()
                            .filter_map(|d| d.as_str())
                            .collect::<Vec<_>>()
                            .join(", "));
                        info.push_str("\n\n");
                    }
                }

                // Main ingredients with amounts
                if let Some(ingredients) = recipe.get("extendedIngredients").and_then(|i| i.as_array()) {
                    info.push_str("📝 Ingredients:\n");
                    for ingredient in ingredients {
                        if let (Some(amount), Some(unit), Some(name), Some(original)) = (
                            ingredient.get("amount").and_then(|a| a.as_f64()),
                            ingredient.get("unit").and_then(|u| u.as_str()),
                            ingredient.get("name").and_then(|n| n.as_str()),
                            ingredient.get("original").and_then(|o| o.as_str()),
                        ) {
                            info.push_str(&format!("• {:.1} {} {} ({})\n", amount, unit, name, original));
                        }
                    }
                    info.push_str("\n");
                }

                // Cooking instructions with steps
                if let Some(instructions) = recipe.get("analyzedInstructions").and_then(|i| i.as_array()) {
                    info.push_str("📋 Instructions:\n");
                    for section in instructions {
                        if let Some(steps) = section.get("steps").and_then(|s| s.as_array()) {
                            for (i, step) in steps.iter().enumerate() {
                                if let Some(step_text) = step.get("step").and_then(|s| s.as_str()) {
                                    info.push_str(&format!("{}. {}\n", i + 1, step_text));
                                }
                            }
                        }
                    }
                    info.push_str("\n");
                }

                // Nutrition information per serving
                if let Some(nutrition) = recipe.get("nutrition") {
                    info.push_str("🥗 Nutrition Facts (per serving):\n");
                    let important_nutrients = [
                        "Calories", "Protein", "Fat", "Carbohydrates",
                        "Fiber", "Sugar", "Sodium", "Cholesterol"
                    ];
                    
                    if let Some(nutrients) = nutrition.get("nutrients").and_then(|n| n.as_array()) {
                        for &nutrient_name in &important_nutrients {
                            if let Some(nutrient) = nutrients.iter().find(|n| {
                                n.get("name").and_then(|name| name.as_str()) == Some(nutrient_name)
                            }) {
                                if let (Some(amount), Some(unit)) = (
                                    nutrient.get("amount").and_then(|a| a.as_f64()),
                                    nutrient.get("unit").and_then(|u| u.as_str()),
                                ) {
                                    info.push_str(&format!("• {}: {:.1} {}\n", nutrient_name, amount, unit));
                                }
                            }
                        }
                    }
                }

                return Ok(info);
            }
        }

        Ok(format!("No recipe found for '{}'. Try:\n1. Check your spelling\n2. Use a more common name\n3. Try a different variation (e.g., 'nasi goreng' for 'indonesian fried rice')\n4. Specify the cuisine type (e.g., 'japanese ramen')", query))
    }
} 