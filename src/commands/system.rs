use colored::Colorize;

pub fn handle_command(input: &str) -> Result<(), String> {
    match input.to_lowercase().as_str() {
        "help" => {
            println!("\n🤖 AI Assistant Commands:");
            println!("  Just type your question or request");
            println!("  Examples:");
            println!("    - show me how to create a web server in rust");
            println!("    - explain error handling in rust");
            println!("    - help me debug this code: [your code]");
            println!();

            println!("👤 Character Commands:");
            println!("  chars         - List available characters");
            println!("  load <name>   - Switch to a different character");
            println!("  Example: load helpful, load friendly");
            println!();

            println!("🔄 Provider Commands:");
            println!("  providers     - List available AI providers");
            println!("  use <name>    - Switch to a different provider");
            println!("  Example: use openai, use openrouter");
            println!();

            println!("🐦 Twitter Commands:");
            println!("  tweet <message>           - Post a tweet");
            println!("  tweet                     - Generate AI tweet");
            println!("  reply <id> <message>      - Reply to a tweet");
            println!("  dm @user: <message>       - Send a direct message");
            println!("  autopost start <minutes>  - Start auto-posting");
            println!("  autopost stop             - Stop auto-posting");
            println!("  logs                      - Show recent activity");
            println!();

            println!("🕷️ Web Commands:");
            println!("  analyze <url>    - Analyze webpage content");
            println!("  research <topic> - Research a topic");
            println!("  links <url>      - Extract links from webpage");
            println!();

            println!("⚙️ System Commands:");
            println!("  help  - Show this help menu");
            println!("  exit  - Exit the program");
            println!();

            println!("📄 Document Commands:");
            println!("  doc analyze <file>   - Analyze a document");
            println!("  doc summary <file>   - Get a quick summary");
            println!("  doc extract <file>   - Extract text from document");
            println!("  doc ocr <image>      - Extract text from image");
            println!("  doc batch <folder>   - Process multiple files");
            println!("  doc info <file>      - Show file information");
            Ok(())
        },
        "exit" | "quit" => {
            println!("👋 Goodbye!");
            std::process::exit(0);
        },
        _ => Err("Unknown system command. Type 'help' for available commands.".to_string())
    }
}