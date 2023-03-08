use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use teloxide::prelude::*;

async fn request_chat_completion(client: &Client, text: &str, model: Option<&str>) -> String {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model.unwrap_or("gpt-3.5-turbo"))
        .messages([ChatCompletionRequestMessageArgs::default()
            .role(Role::User)
            .content(text)
            .build()
            .unwrap()])
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

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let client = client.clone();

        async move {
            let text = msg.text().unwrap();
            log::info!("Receive message user: {}, content: {}", msg.chat.id, text);

            let response = request_chat_completion(&client, text, None).await;

            bot.send_message(msg.chat.id, response).await?;

            Ok(())
        }
    })
    .await;
}
