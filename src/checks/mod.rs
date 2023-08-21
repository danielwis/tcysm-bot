use crate::{Context, Error};

pub async fn check_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let is_admin = ctx
        .author_member()
        .await
        .unwrap()
        .roles
        .contains(&ctx.data().admin_role_id);

    if !is_admin {
        ctx.say("Mod privileges required to issue this command")
            .await?;
    }

    Ok(is_admin)
}
