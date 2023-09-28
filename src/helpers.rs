use crate::{Context, Error};

pub async fn post_to_modlog(ctx: Context<'_>, msg: &str) -> Result<(), Error> {
    // `say()` returns a Result<message, Err> here but we don't care about the message
    ctx.data()
        .modlog_channel_id
        .say(ctx, msg)
        .await
        .map(|_| Ok(()))?
}
