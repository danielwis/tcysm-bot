mod commands;
mod meta;

use poise::serenity_prelude as serenity;
pub struct Data {}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![meta::source(), meta::register(), commands::authenticate()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .token(
            std::env::var("DISCORD_TOKEN")
                .expect("Unable to find Discord token in environment variables"),
        )
        .intents(
            serenity::GatewayIntents::GUILD_MESSAGES
                | serenity::GatewayIntents::DIRECT_MESSAGES
                | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                println!("Connected as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .run()
        .await
        .unwrap();
}
