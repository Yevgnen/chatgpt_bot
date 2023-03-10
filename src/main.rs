use async_openai::types::ChatCompletionRequestMessage;
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use futures::StreamExt;
use std::error::Error;
use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};
use teloxide::{prelude::*, utils::command::BotCommands};

type ChatMessages = Vec<ChatCompletionRequestMessage>;
type ChatHistories = HashMap<ChatId, ChatMessages>;
type State = Arc<Mutex<ChatHistories>>;
type HandleResult = Result<(), Box<dyn Error + Send + Sync>>;

const MODEL: &str = "gpt-3.5-turbo";

async fn complete_chat(
    content: String,
    bot: Bot,
    client: Client,
    state: State,
    msg: Message,
) -> HandleResult {
    log::info!("Complete chat, user: {}, content: {}", msg.chat.id, content);

    let hists;
    {
        let mut guard = state.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::User)
                .content(content)
                .build()
                .unwrap(),
        );
        hists = messages.clone();
    }

    let response = bot
        .send_message(msg.chat.id, "💭")
        .reply_to_message_id(msg.id)
        .await
        .unwrap();
    let msg_id = response.id;

    let request = CreateChatCompletionRequestArgs::default()
        .model(MODEL)
        .messages(hists)
        .build()
        .unwrap();
    let mut stream = client.chat().create_stream(request).await?;

    let mut chunks = Vec::new();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        if let Some(ref content) = result.unwrap().choices.get(0).unwrap().delta.content {
            chunks.push(content.to_owned());
            if !content.trim().is_empty() {
                count += 1;
                if count % 20 == 0 {
                    bot.edit_message_text(msg.chat.id, msg_id, chunks.join(""))
                        .await
                        .unwrap();
                }
            }
        }
    }
    bot.edit_message_text(msg.chat.id, msg_id, chunks.join(""))
        .await
        .unwrap();

    {
        let mut guard = state.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::Assistant)
                .content(chunks.join(""))
                .build()
                .unwrap(),
        );
    }

    Ok(())
}

async fn set_prompt(prompt: String, bot: Bot, state: State, msg: Message) -> HandleResult {
    log::info!("Set prompt, user: {}, prompt: {}", msg.chat.id, prompt);

    {
        let mut guard = state.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        messages.clear();
        messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::System)
                .content(prompt)
                .build()
                .unwrap(),
        );
    }

    bot.send_message(msg.chat.id, "Prompt set.")
        .reply_to_message_id(msg.id)
        .await?;

    Ok(())
}

async fn view_histories(bot: Bot, state: State, msg: Message) -> HandleResult {
    let content = {
        let mut guard = state.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        if messages.is_empty() {
            "Empty chat history.".to_owned()
        } else {
            messages
                .iter()
                .map(|msg| format!("{}: {}", msg.role, msg.content.trim()))
                .collect::<Vec<String>>()
                .join("\n\n")
        }
    };

    bot.send_message(msg.chat.id, content)
        .reply_to_message_id(msg.id)
        .await?;

    Ok(())
}

async fn clear_history(bot: Bot, state: State, msg: Message) -> HandleResult {
    {
        let mut guard = state.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        messages.clear();
    }

    bot.send_message(msg.chat.id, "Chat histories cleared.")
        .reply_to_message_id(msg.id)
        .await?;

    Ok(())
}

async fn handle_command(
    bot: Bot,
    client: Client,
    state: State,
    msg: Message,
    cmd: Command,
) -> HandleResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Prompt(prompt) => {
            set_prompt(prompt, bot, state, msg).await?;
        }
        Command::Chat(content) => {
            complete_chat(content, bot, client, state, msg).await?;
        }
        Command::View => {
            view_histories(bot, state, msg).await?;
        }
        Command::Clear => {
            clear_history(bot, state, msg).await?;
        }
    }
    Ok(())
}

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "set prompt text.")]
    Prompt(String),
    #[command(description = "chat with gpt.")]
    Chat(String),
    #[command(description = "view chat histories.")]
    View,
    #[command(description = "clear history chats.")]
    Clear,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();

    let client = Client::new();
    let state = Arc::new(Mutex::new(ChatHistories::new()));

    let handler = Update::filter_message().branch(
        dptree::entry()
            .filter_command::<Command>()
            .endpoint(handle_command),
    );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![client, state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
