use crate::providers::web_crawler::crawler_manager::WebCrawlerManager;
use crate::providers::traits::CompletionProvider;
use crate::llm::memory::MemoryManager;
use colored::Colorize;

pub async fn handle_command(
    input: &str,
    crawler: &WebCrawlerManager,
    provider: &Box<dyn CompletionProvider + Send + Sync>,
    memory_manager: &mut MemoryManager,
) -> Result<String, String> {
    match input {
        s if s.starts_with("analyze ") => {
            let url = s.trim_start_matches("analyze ").trim();
            if url.is_empty() {
                println!("Please provide a URL to analyze.");
                println!("Usage: analyze <url>");
                return Ok("Please provide a URL to analyze.".to_string());
            }

            let content = crawler.analyze_url(url).await
                .map_err(|e| format!("Failed to analyze webpage: {}", e))?;

            // Store webpage content in memory
            let context = format!("Webpage being discussed: {}\nContent:\n{}", url, content);
            let embedding = generate_embedding(&context).await?;
            memory_manager.store_memory(&context, "webpage", embedding, None)
                .await
                .map_err(|e| format!("Failed to store memory: {}", e))?;

            // Create personality-aware analysis prompt
            let analysis_prompt = format!(
                "{}\n\nAs this character, analyze and synthesize this webpage content and provide your unique perspective. \
                find the key point , Consider your personality traits and expertise when providing this analysis. \
                Be creative and stay true to your character's style:\n\n{}",
                provider.get_system_message(),
                content
            );

            let analysis = provider.complete(&analysis_prompt).await
                .map_err(|e| format!("Failed to analyze content: {}", e))?;

            // Store analysis in memory
            let analysis_context = format!("Analysis of webpage: {}\n{}", url, analysis);
            let embedding = generate_embedding(&analysis_context).await?;
            memory_manager.store_memory(&analysis_context, "analysis", embedding, None)
                .await
                .map_err(|e| format!("Failed to store memory: {}", e))?;

            println!("\n📊 Analysis Results for {}:", url.bright_yellow());
            println!("{}", analysis.truecolor(255, 236, 179));
            println!("\n💭 You can now ask questions about this webpage. Try:");
            println!("  web chat what are the main points?");
            println!("  web chat can you explain [specific topic] in more detail?");
            Ok("Analysis complete.".to_string())
        },
        s if s.starts_with("research ") => {
            let topic = s.trim_start_matches("research ").trim();
            if topic.is_empty() {
                println!("Please provide a topic to research.");
                println!("Usage: research <topic>");
                return Ok("Please provide a topic to research.".to_string());
            }

            let results = crawler.research_topic(topic).await
                .map_err(|e| format!("Failed to research topic: {}", e))?;

            // Store research results in memory
            let context = format!("Research topic: {}\nResearch findings:\n{}", topic, results.join("\n"));
            let embedding = generate_embedding(&context).await?;
            memory_manager.store_memory(&context, "research", embedding, None)
                .await
                .map_err(|e| format!("Failed to store memory: {}", e))?;

            // Create personality-aware research prompt with better structure
            let research_prompt = format!(
                "{}\n\n\
                As this character, analyze and synthesize the research about '{}'in your unique style. \
                Structure your response in these sections:\n\
                1. Key Findings (3-10 main points)\n\
                2. Analysis with (your unique perspective)\n\
                Keep each section focused and insightfull \
                Stay true to your character's expertise and communication style.\n\n\
                3.then make quick summarize all of these , short and insightfull and adviceswith your own unique style:\n{}", 
                provider.get_system_message(),
                topic,
                results.join("\n")
            );

            let analysis = provider.complete(&research_prompt).await
                .map_err(|e| format!("Failed to synthesize research: {}", e))?;

            // Store analysis in memory
            let analysis_context = format!("Research analysis: {}\n{}", topic, analysis);
            let embedding = generate_embedding(&analysis_context).await?;
            memory_manager.store_memory(&analysis_context, "analysis", embedding, None)
                .await
                .map_err(|e| format!("Failed to store memory: {}", e))?;

            println!("\n📚 Research Results for '{}':", topic.bright_yellow());
            println!("{}", analysis.truecolor(255, 236, 179));
            println!("\n💭 You can now ask questions about this research. Try:");
            println!("  web chat tell me more about [specific finding]");
            println!("  web chat what are the implications of [topic]?");
            Ok("Research complete.".to_string())
        },
        s if s.starts_with("links ") => {
            let url = s.trim_start_matches("links ").trim();
            if url.is_empty() {
                println!("Please provide a URL to extract links from.");
                println!("Usage: links <url>");
                return Ok("Please provide a URL to extract links from.".to_string());
            }

            let links = crawler.extract_links(url).await
                .map_err(|e| format!("Failed to extract links: {}", e))?;

            println!("\n🔗 Links from {}:", url.bright_yellow());
            let link_count = links.len();
            for link in links {
                println!("• {}", link);
            }
            println!("\n📊 Total links found: {}", link_count);
            Ok("Links extracted.".to_string())
        },
        s if s.starts_with("chat ") => {
            let query = s.trim_start_matches("chat ").trim();

            // Generate embedding for the query
            let query_embedding = generate_embedding(query).await?;
            
            // Search for relevant memories
            let memories = memory_manager.search_similar(query_embedding, 5).await
                .map_err(|e| format!("Failed to search memories: {}", e))?;
            
            // Build context from memories
            let context = memory_manager.summarize_memories(&memories).await;

            // Create chat prompt with context
            let chat_prompt = format!(
                "{}\n\n\
                Previous context:\n{}\n\n\
                User question: {}\n\n\
                Answer the question based on the previous context while maintaining your character's personality. \
                Keep your response focused and relevant to the topic being discussed.",
                provider.get_system_message(),
                context,
                query
            );

            let response = provider.complete(&chat_prompt).await
                .map_err(|e| format!("Failed to get response: {}", e))?;

            // Store the chat interaction
            let interaction = format!("Q: {}\nA: {}", query, response);
            let embedding = generate_embedding(&interaction).await?;
            memory_manager.store_memory(&interaction, "chat", embedding, None)
                .await
                .map_err(|e| format!("Failed to store memory: {}", e))?;

            println!("\n💬 Response:");
            println!("{}", response.bright_green());
            Ok("Chat completed.".to_string())
        },
        _ => Err("Unknown web command. Available commands:\n  analyze <url> - Analyze webpage content\n  research <topic> - Research a topic\n  links <url> - Extract links from webpage".to_string())
    }
}

async fn generate_embedding(text: &str) -> Result<Vec<f32>, String> {
    // This is a placeholder - you should implement actual embedding generation
    // For now, return a dummy embedding of size 1536 (OpenAI's embedding size)
    Ok(vec![0.0; 1536])
}
