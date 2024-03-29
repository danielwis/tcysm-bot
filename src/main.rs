mod auth;
mod checks;
mod helpers;
mod meta;

use lettre::{transport::smtp::authentication::Credentials, SmtpTransport};
use poise::serenity_prelude as serenity;
pub struct Data {
    database: sqlx::SqlitePool,
    mailer: SmtpTransport,
    admin_role_id: serenity::RoleId,
    modlog_channel_id: serenity::ChannelId,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Failed to find .env file in current directory.");

    // Setup email
    let smtp_username =
        std::env::var("SMTP_USER").expect("Missing SMTP username in environment variables");
    let smtp_password =
        std::env::var("SMTP_PASS").expect("Missing SMTP password in environment variables");
    let smtp_server =
        std::env::var("SMTP_SERVER").expect("Missing SMTP server address in environment variables");

    let creds = Credentials::new(smtp_username, smtp_password);
    let mailer = SmtpTransport::starttls_relay(&smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Setup local database
    let db_url = std::env::var("DATABASE_URL").unwrap();
    let database = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            db_url
                .parse::<sqlx::sqlite::SqliteConnectOptions>()
                .unwrap()
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&database).await.unwrap();

    // Setup server/guild related variables
    let admin_role_id = std::env::var("ADMIN_ROLE_ID")
        .expect("Unable to find admin role ID in environment variables")
        .parse::<serenity::RoleId>()
        .unwrap();

    let modlog_channel_id = std::env::var("MODLOG_CHANNEL_ID")
        .expect("Unable to find modlog channel ID in environment variables")
        .parse::<serenity::ChannelId>()
        .unwrap();

    poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                meta::source(),
                meta::register(),
                meta::test_modlog(),
                auth::authenticate(),
                auth::passreg(),
            ],
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
                | serenity::GatewayIntents::MESSAGE_CONTENT
                | serenity::GatewayIntents::GUILDS,
        )
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                println!("Connected as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    database,
                    mailer,
                    admin_role_id,
                    modlog_channel_id
                })
            })
        })
        .run()
        .await
        .unwrap();
}
