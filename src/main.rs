use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};

use async_openai::types::ChatCompletionRequestMessage;
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use teloxide::prelude::*;

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

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();
    let client = Client::new();

    let chat_histories = Arc::new(Mutex::new(ChatHistories::new()));

    let handler = Update::filter_message().endpoint(
        move |bot: Bot, chat_histories: Arc<Mutex<ChatHistories>>, msg: Message| {
            let client = client.clone();

            let content = msg.text().unwrap();
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

            async move {
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

                bot.send_message(msg.chat.id, response).await?;

                respond(())
            }
        },
    );

    Dispatcher::builder(bot, handler)
        // Pass the shared state to the handler as a dependency.
        .dependencies(dptree::deps![chat_histories])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
