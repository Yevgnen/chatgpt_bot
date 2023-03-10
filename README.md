# Table of Contents <span class="tag" tag-name="TOC"><span class="smallcaps">TOC</span></span>

- [Introduction](#introduction)
- [Build & Run](#build--run)
- [Support commands](#support-commands)

# Introduction

Simple ChatGPT Telegram bot written in Rust.

# Build & Run

``` rustic
OPENAI_API_KEY=[YOUR_API_KEY] TELOXIDE_TOKEN=[YOUR_TOKEN] cargo run --release
```

Replace `[YOUR_API_KEY]` with your OpenAI API key and `[TELOXIDE_TOKEN]` with you bot token.

# Support commands

Type `/help` the chat window to see supported commands:

``` example
These commands are supported:

/help — display this text.
/prompt — set prompt text.
/chat — chat with gpt.
/view — view chat histories.
/clear — clear history chats.
```
