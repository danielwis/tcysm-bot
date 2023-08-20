use crate::{Context, Error};

use crate::checks::check_admin;

#[poise::command(slash_command)]
pub async fn source(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("https://github.com/danielwis/tcysm-bot").await?;
    Ok(())
}

#[poise::command(prefix_command, check = "check_admin")]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

