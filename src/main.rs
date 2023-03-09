use std::error::Error;
use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};

use async_openai::types::ChatCompletionRequestMessage;
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use teloxide::{prelude::*, utils::command::BotCommands};

type ChatMessages = Vec<ChatCompletionRequestMessage>;
type ChatHistories = HashMap<ChatId, ChatMessages>;
type ChatHistoryState = Arc<Mutex<ChatHistories>>;
type HandleResult = Result<(), Box<dyn Error + Send + Sync>>;

async fn request_chat_completion(
    client: &Client,
    messages: ChatMessages,
    model: Option<&str>,
) -> String {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model.unwrap_or("gpt-3.5-turbo"))
        .messages(messages)
        .build()
        .unwrap();

    let response = client.chat().create(request).await.unwrap();

    response.choices.get(0).unwrap().message.content.clone()
}

async fn complete_chat(
    bot: Bot,
    client: Client,
    chat_histories: ChatHistoryState,
    msg: Message,
    content: String,
) -> HandleResult {
    log::info!(
        "Receive message user: {}, content: {}",
        msg.chat.id,
        content
    );

    let hists;
    {
        let mut guard = chat_histories.lock().unwrap();
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

    let response = request_chat_completion(&client, hists, None).await;

    {
        let mut guard = chat_histories.lock().unwrap();
        let messages = guard.entry(msg.chat.id).or_default();
        messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::Assistant)
                .content(response.clone())
                .build()
                .unwrap(),
        );
    }

    bot.send_message(msg.chat.id, response)
        .reply_to_message_id(msg.id)
        .await?;

    Ok(())
}

async fn set_prompt(
    bot: Bot,
    chat_histories: ChatHistoryState,
    msg: Message,
    prompt: String,
) -> HandleResult {
    log::info!("Set prompt user: {}, prompt: {}", msg.chat.id, prompt);

    {
        let mut guard = chat_histories.lock().unwrap();
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

async fn view_histories(bot: Bot, chat_histories: ChatHistoryState, msg: Message) -> HandleResult {
    let content = {
        let mut guard = chat_histories.lock().unwrap();
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

async fn clear_history(bot: Bot, chat_histories: ChatHistoryState, msg: Message) -> HandleResult {
    {
        let mut guard = chat_histories.lock().unwrap();
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
    chat_histories: ChatHistoryState,
    msg: Message,
    cmd: Command,
) -> HandleResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Prompt(prompt) => {
            set_prompt(bot, chat_histories, msg, prompt).await?;
        }
        Command::Chat(content) => {
            complete_chat(bot, client, chat_histories, msg, content).await?;
        }
        Command::View => {
            view_histories(bot, chat_histories, msg).await?;
        }
        Command::Clear => {
            clear_history(bot, chat_histories, msg).await?;
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
    let chat_histories = Arc::new(Mutex::new(ChatHistories::new()));

    let handler = Update::filter_message().branch(
        dptree::entry()
            .filter_command::<Command>()
            .endpoint(handle_command),
    );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![client, chat_histories])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
