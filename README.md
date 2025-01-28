# AiRysZ - Deepseek 

Xperimental project - Warning 

"I'm starting my first AI project "

"This is for personal documentation and learning purposes only. im not Developer ,I don't have coding experience, and neither do the others here. However, feel free to try it out if you're interested."

==============================

to start 

Get your Api key from deepseek 
https://platform.deepseek.com/

just only $2 for activated this API

and create .env in you root dir

# DeepSeek Configuration 
DEEPSEEK_API_KEY=

DEEPSEEK_BASE_URL=https://api.deepseek.com

DEEPSEEK_MODEL=deepseek-chat

DEEPSEEK_MAX_TOKENS=2048

DEEPSEEK_TEMPERATURE=0.7

and then 

cargo Run 

happy to chat with ur own deepseek 

# Example result 

![Screenshot_2025-01-20-10-22-56-081_com twitter android-edit](https://github.com/user-attachments/assets/3fe5c782-f4d1-443d-b9d4-52b84b2f4d13)

![Screenshot_2025-01-20-10-24-02-172_com twitter android-edit](https://github.com/user-attachments/assets/25beeb4b-c723-4b50-ae99-cd6e743d4d00)

![Screenshot_2025-01-20-10-24-21-800_com twitter android-edit](https://github.com/user-attachments/assets/cc4b810a-4cc1-47db-b11a-fd24a63b0026)


can change character in runtime too

![Screenshot_2025-01-20-10-51-14-485_com twitter android-edit](https://github.com/user-attachments/assets/5608d9a9-6755-4d70-978d-826618b66acb)


======================

# AiRysZ

## 🚀 Project Overview

### Vision
An advanced, modular AI agent built in Rust, designed to provide intelligent, context-aware, and dynamically adaptive conversational experiences.

## 🧠 Core Features

### 1. Dynamic Personality System
- **Modular Character Profiles**
  - JSON-based personality configuration
  - Rich emotional expression capabilities
  - Customizable communication styles

### 2. Intelligent Conversation Management
- **Persistent Memory Storage**
  - SQLite-powered conversation tracking
  - Context retention and learning
  - Dynamic knowledge expansion

### 3. Emotional Intelligence
- **Emoji and Emote Support**
  - Context-specific emotional expressions
  - Adaptive communication strategies
  - Enhanced interaction depth

## 🔧 Technical Architecture

### Language and Technologies
- **Primary Language**: Rust
- **Database**: SQLite (rusqlite)
- **Serialization**: Serde
- **Character Management**: JSON-based configuration

### Key Components
- Personality Loader, u can change character in Runtime 
- Conversation Tracker
- Emotion Expression Engine
  
======≠===============

- Support Twitter integration

- Support Web Crawler
(Research topic ,analyze url , Find info links ) 

- Support Document Processor
  (all format , but not all doc work well , use with caution) 

## 🤝 Contribution
fell free  

## 💡 Getting Started
```bash
# Clone the repository
git clone https://github.com/https://github.com/AiRysZ-Agent/AiRysZ-Agents

# Build the project
cargo build

# Run the AI agent
cargo run

## 🎭 Loading Characters ( Im Inspired by ElizaOS ) The best role Model

### Character Selection Methods

#### 1. Interactive Character Selection
When you run the AI agent, you'll see a prompt to choose a character:
Available Characters:

Type 'coding_ninja' for Zara "CodeWizard" Chen
Type 'academic_researcher' for Dr. Rissa
Type 'masterchef_scientist' for Joey
Type 'startup_founder' for Alex Chen


#### 2. Direct Filename Loading
You can load any character by typing its filename:
```bash
# Load a character directly by filename
masterchef_scientist.json

3. Programmatic Character Loading
In your Rust code, you can load characters programmatically:

// Create a new character dynamically

let custom_character = PersonalityProfile {
    name: "Custom Character".to_string(),

// Add more custom configuration
};

{
    "name": "Your Character Name",
    "bio": { ... },
    "traits": { ... },
    "emotions": {
        "expressions": {
            "emotion_name": {
                "emojis": ["😄", "🚀"],
                "emotes": ["*does something*"]
            }
        }
    }
}

Best Practices
Keep character files in characters/ directory

Use meaningful, descriptive filenames

Maintain consistent JSON structure

Experiment with different personality traits

==============
NOTED

- For Front End still on Progress

- Have Many Warning ⚠️ ⚠️ , use with Caution 🫡🫡🙏

- Messy Documentations 😄 ( still working ) 

==============

inspired by @elizaOs , @RiG playground , @ZereBro , and others Ai Agentz , 2025 is years Of Ai Agent , lets go 🔥 🔥

=========================

## UPDATE 2025-01-24

## 🚀 New Features

### 1. Multi-LLM Support
- Multiple LLM providers integration:
  - OpenAI 
  - OpenRouter
  - Mistral AI
  - Google Gemini
  - DeepSeek (default )

- Automatic fallback mechanism between providers

- Dynamic model switching based on availability and performance

### 2. Vector Database Integration
- Integrated Qdrant vector database for efficient similarity search
- Persistent storage of conversation history and embeddings
- Configurable collection management
- Automatic cleanup of old entries

### 3. Enhanced Memory Management
- Improved context handling with sliding window
- Token-aware memory management
- Automatic cleanup of old memories
- Configurable memory limits and cleanup intervals
- Session-based conversation management

### 4. Food Mode Features
- Recipe generation and analysis ( Using spoonacular API )
- Nutritional information lookup
- Ingredient substitution suggestions
- Dietary restriction handling
- Meal planning assistance

### 5. Update Document Processing
- Automatic text extraction and processing
- Vector embedding for document content
- Semantic search across documents

## 🛠 Setup

### Prerequisites
- Rust (latest stable version)
- Qdrant vector database
- API keys for desired LLM providers



## 🚀 Usage

### Basic Usage
```bash
cargo run -- --provider deepseek
```

### With Food Mode
```bash
cargo run -- --provider deepseek --food-mode
```

### With Web Crawler
```bash
cargo run -- --provider deepseek --crawler
```

### API Server Mode
```bash
cargo run -- --api --port 3000
```

### COMBINED MODE
```bash
cargo run -- --provider deepseek --crawler --twitter --character (yours character name json )
```

## 📝 Command Examples

### Chat Commands
```
> help                    # Show available commands
> use openai             # Switch to OpenAI provider
> use mistral            # Switch to Mistral provider
```

### Food Mode Commands
```
> recipe pasta           # Generate pasta recipe
> nutrition apple        # Get nutritional info for apple
```

## NOTED 
OPEN AI KEY MUST BE ACTIVATED ( FOR EMBEDDING )
