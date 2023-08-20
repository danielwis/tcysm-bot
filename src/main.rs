mod auth;
mod checks;
mod meta;

use poise::serenity_prelude as serenity;
pub struct Data {
    database: sqlx::SqlitePool,
    admin_role_id: serenity::RoleId,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Failed to find .env file in current directory.");

    let db_url = std::env::var("DATABASE_URL").unwrap();
    let database = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            db_url
                .parse::<sqlx::sqlite::SqliteConnectOptions>()
                .unwrap()
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&database).await.unwrap();

    let admin_role_id = std::env::var("ADMIN_ROLE_ID")
        .unwrap()
        .parse::<serenity::RoleId>()
        .unwrap();

    poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![meta::source(), meta::register(), auth::authenticate()],
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
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                println!("Connected as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    database,
                    admin_role_id,
                })
            })
        })
        .run()
        .await
        .unwrap();
}
