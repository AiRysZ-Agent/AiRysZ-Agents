#[derive(Debug)]
pub struct UsdaClient {
    api_key: String,
    base_url: String,
}

// Common recipe ingredients mapping
const RECIPE_COMPONENTS: &[(&str, &[&str])] = &[
    ("spaghetti carbonara", &["spaghetti pasta", "eggs", "pecorino cheese", "black pepper", "pancetta"]),
    ("japanese ramen", &["ramen noodles", "chicken broth", "soy sauce", "green onions"]),
    ("pizza", &["pizza dough", "tomato sauce", "mozzarella cheese"]),
];

impl UsdaClient {
    pub fn new(config: crate::food::config::FoodConfig) -> Self {
        Self {
            api_key: config.usda_api_key,
            base_url: "https://api.nal.usda.gov/fdc/v1".to_string(),
        }
    }

    pub async fn search_food(&self, query: &str) -> Result<String, String> {
        // Check if this is a complex dish that needs to be broken down
        if let Some(components) = RECIPE_COMPONENTS.iter().find(|(dish, _)| query.to_lowercase().contains(&dish.to_lowercase())) {
            return self.analyze_recipe_components(components.0, components.1).await;
        }

        // Original single ingredient search
        self.search_single_food(query).await
    }

    async fn analyze_recipe_components(&self, dish_name: &str, components: &[&str]) -> Result<String, String> {
        let mut combined_info = format!("Food: {} (Recipe Breakdown)\n\nIngredient Analysis:\n", dish_name);
        
        for ingredient in components {
            match self.search_single_food(ingredient).await {
                Ok(info) => {
                    combined_info.push_str(&format!("\n=== {} ===\n{}\n", ingredient, info));
                }
                Err(e) => {
                    combined_info.push_str(&format!("\n=== {} ===\nCould not find data: {}\n", ingredient, e));
                }
            }
        }

        Ok(combined_info)
    }

    async fn search_single_food(&self, query: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let url = format!("{}/foods/search", self.base_url);
        
        // Try different data types to get better results
        let data_types = ["Survey (FNDDS)", "Foundation", "SR Legacy"];
        let mut best_match: Option<(String, serde_json::Value)> = None;
        
        for data_type in data_types.iter() {
            let response = client
                .get(&url)
                .query(&[
                    ("api_key", &self.api_key),
                    ("query", &query.to_string()),
                    ("dataType", &data_type.to_string()),
                    ("pageSize", &"10".to_string()),
                ])
                .send()
                .await
                .map_err(|e| format!("Failed to send request: {}", e))?;

            if !response.status().is_success() {
                continue;
            }

            let data: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            if let Some(foods) = data.get("foods").and_then(|f| f.as_array()) {
                for food in foods {
                    if let Some(description) = food.get("description").and_then(|d| d.as_str()) {
                        let is_better_match = match &best_match {
                            None => true,
                            Some((curr_desc, _)) => {
                                let curr_score = string_similarity(curr_desc, query);
                                let new_score = string_similarity(description, query);
                                new_score > curr_score
                            }
                        };

                        if is_better_match {
                            best_match = Some((description.to_string(), food.clone()));
                        }
                    }
                }
            }
        }

        if let Some((_description, food)) = best_match {
            let mut nutrition_info = String::new();
            
            if let Some(nutrients) = food.get("foodNutrients").and_then(|n| n.as_array()) {
                for nutrient in nutrients {
                    if let (Some(name), Some(amount), Some(unit)) = (
                        nutrient.get("nutrientName").and_then(|n| n.as_str()),
                        nutrient.get("value").and_then(|v| v.as_f64()),
                        nutrient.get("unitName").and_then(|u| u.as_str()),
                    ) {
                        nutrition_info.push_str(&format!("- {}: {:.1} {}\n", name, amount, unit));
                    }
                }
            }

            return Ok(nutrition_info);
        }

        Ok(format!("No nutrition data found for '{}'", query))
    }
}

fn string_similarity(s1: &str, s2: &str) -> f64 {
    let s1_lower = s1.to_lowercase();
    let s2_lower = s2.to_lowercase();
    
    let s1_words: Vec<&str> = s1_lower.split_whitespace().collect();
    let s2_words: Vec<&str> = s2_lower.split_whitespace().collect();
    
    let matches = s1_words.iter()
        .filter(|w| s2_words.contains(w))
        .count();
    
    matches as f64 / s1_words.len().max(s2_words.len()) as f64
}