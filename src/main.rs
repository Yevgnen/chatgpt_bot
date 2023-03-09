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
    chat_histories: Arc<Mutex<ChatHistories>>,
    id: ChatId,
    content: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Receive message user: {}, content: {}", id, content);

    let hists;
    {
        let mut guard = chat_histories.lock().unwrap();
        let messages = guard.entry(id).or_default();
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
        let messages = guard.entry(id).or_default();
        messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::Assistant)
                .content(response.clone())
                .build()
                .unwrap(),
        );
    }

    bot.send_message(id, response).await?;

    Ok(())
}

fn set_prompt(chat_histories: Arc<Mutex<ChatHistories>>, id: ChatId, prompt: String) {
    log::info!("Set prompt user: {}, prompt: {}", id, prompt);

    let mut guard = chat_histories.lock().unwrap();
    let messages = guard.entry(id).or_default();
    messages.clear();
    messages.push(
        ChatCompletionRequestMessageArgs::default()
            .role(Role::System)
            .content(prompt)
            .build()
            .unwrap(),
    );
}

async fn view_histories(
    bot: Bot,
    chat_histories: Arc<Mutex<ChatHistories>>,
    id: ChatId,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let content = {
        let mut guard = chat_histories.lock().unwrap();
        let messages = guard.entry(id).or_default();
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

    bot.send_message(id, content).await?;

    Ok(())
}

fn clear_history(chat_histories: Arc<Mutex<ChatHistories>>, id: ChatId) {
    let mut guard = chat_histories.lock().unwrap();
    let messages = guard.entry(id).or_default();
    messages.clear();
}

async fn handle_command(
    bot: Bot,
    client: Client,
    chat_histories: Arc<Mutex<ChatHistories>>,
    msg: Message,
    cmd: Command,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Prompt(prompt) => {
            set_prompt(chat_histories, msg.chat.id, prompt);
        }
        Command::Chat(content) => {
            complete_chat(bot, client, chat_histories, msg.chat.id, content).await?;
        }
        Command::View => {
            view_histories(bot, chat_histories, msg.chat.id).await?;
        }
        Command::Clear => {
            clear_history(chat_histories, msg.chat.id);
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
