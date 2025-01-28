#[derive(Debug, Clone)]
pub struct FoodConfig {
    pub usda_api_key: String,
    pub spoonacular_api_key: String,
}

impl FoodConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            usda_api_key: std::env::var("USDA_API_KEY")
                .map_err(|_| "USDA_API_KEY environment variable not set".to_string())?,
            spoonacular_api_key: std::env::var("SPOONACULAR_API_KEY")
                .map_err(|_| "SPOONACULAR_API_KEY environment variable not set".to_string())?,
        })
    }
}