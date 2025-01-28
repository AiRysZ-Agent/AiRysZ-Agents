use crate::providers::twitter::manager::ConversationManager;

pub async fn handle_command(
    input: &str,
    manager: &mut Option<ConversationManager>
) -> Result<(), String> {
    if let Some(ref mut manager) = manager {
        if input.trim() == "tweet" {
            println!("🤖 Generating AI tweet...");
            match manager.handle_command(input).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.to_string().contains("DEEPSEEK_API_KEY") {
                        println!("❌ AI tweet generation requires the DEEPSEEK_API_KEY environment variable to be set.");
                        println!("Please set it and try again, or use 'tweet <message>' to post a manual tweet.");
                    } else {
                        println!("❌ Failed to generate AI tweet: {}", e);
                    }
                    Ok(())
                }
            }
        } else {
            manager.handle_command(input).await
                .map_err(|e| format!("Twitter error: {}", e))
        }
    } else {
        Err("Twitter functionality not enabled. Run with --twitter flag to enable.".to_string())
    }
}