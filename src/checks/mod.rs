use crate::{Context, Error};

pub async fn check_admin(ctx: Context<'_>) -> Result<bool, Error> {
    Ok(ctx
        .author_member()
        .await
        .unwrap()
        .roles
        .contains(&ctx.data().admin_role_id))
}
