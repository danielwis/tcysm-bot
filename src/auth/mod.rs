use crate::checks::check_admin;
use crate::{Context, Error};
use lettre::Transport;
use lettre::{message::header::ContentType, Message};
use poise::serenity_prelude::{self as serenity, CacheHttp};
use rand::distributions::Alphanumeric;
use rand::distributions::DistString;
use reqwest;
use scraper::{Html, Selector};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct KTHUser {
    #[serde(rename = "mail")]
    email: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

async fn get_employee_uids() -> Result<Vec<String>, Error> {
    let html = reqwest::get("https://www.kth.se/directory/j/jh").await;
    if let Err(why) = html {
        println!("Error getting employee list: {}", why);
        return Err(Box::from(why));
    };

    let parsed_html = Html::parse_document(&html?.text().await?);
    let selector = Selector::parse(
        // r#"main > div > div > div > div > div > div > table > tbody > tr > td > a"#,
        r#"div > table > tbody > tr > td.email > a"#,
    )
    .unwrap();

    // if let Err(why) = selector {
    //     println!("{}", why);
    // }

    let mut emp_ids: Vec<String> = vec![];
    for elem in parsed_html.select(&selector) {
        let addr = elem.inner_html().to_string();
        let uid = addr.split("@").next().unwrap();
        emp_ids.push(uid.to_string());
    }

    return Ok(emp_ids);
}

async fn decide_role_to_grant(kth_id: &str, ctx: Context<'_>) -> Result<serenity::RoleId, Error> {
    // TODO
    let curr_guild = ctx.guild().unwrap();

    let teacher_role_id = curr_guild
        .role_by_name("Teacher")
        .ok_or("Failed to find the required roles in server.")?
        .id;
    let student_role_id = curr_guild
        .role_by_name("Student")
        .ok_or("Failed to find the required roles in server.")?
        .id;

    return if get_employee_uids().await?.contains(&kth_id.to_string()) {
        Ok(teacher_role_id)
    } else {
        Ok(student_role_id)
    };
}

async fn send_authentication_email(
    ctx: Context<'_>,
    user: KTHUser,
    auth_code: &str,
) -> Result<(), Error> {
    let message = format!("Hello, this is your code: {auth_code}");
    let email = Message::builder()
        .from("Daniel Williams<dwilli@kth.se>".parse()?)
        // TODO: Get user name too, so that it can be entered in the same format
        .to(format!("{}<{}>", user.display_name, user.email).parse()?)
        .subject("TCYSM Discord authentication")
        .header(ContentType::TEXT_PLAIN)
        .body(message)
        .unwrap();

    ctx.data().mailer.send(&email)?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, subcommands("id", "passphrase"))]
pub async fn authenticate(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Invalid use of parent command.").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, ephemeral = true, guild_only)]
pub async fn passphrase(
    ctx: Context<'_>,
    #[description = "The passphrase"] passphrase: String,
) -> Result<(), Error> {
    let db = &ctx.data().database;
    let mut transaction = db.begin().await?;

    let matching_roles: Vec<serenity::RoleId> = sqlx::query!(
        "SELECT role FROM linked_roles WHERE passphrase = ?;",
        passphrase
    )
    .fetch_all(&mut *transaction)
    .await?
    .iter()
    .filter_map(|row| row.role.parse::<serenity::RoleId>().ok())
    .collect();

    if matching_roles.is_empty() {
        ctx.say("This phrase does not currently seem to be linked to any roles. Please try again.")
            .await?;
        return Ok(());
    }

    for role in &matching_roles {
        // Add role to user
        ctx.author_member()
            .await
            .unwrap()
            .to_mut()
            .add_role(&ctx.serenity_context().http, role)
            .await?;
    }

    transaction.commit().await?;

    ctx.say("Authentication successful. If you were missing any roles linked to the passphrase, these have now been given to you.").await?;

    Ok(())
}

#[poise::command(slash_command, subcommands("begin", "verify"))]
pub async fn id(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Invalid use of parent command.").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    ephemeral = true,
    guild_only,
    check = "check_admin"
)]
pub async fn begin(
    ctx: Context<'_>,
    #[description = "Your KTH ID"] kth_id: String,
) -> Result<(), Error> {
    let author_id = ctx.author().id.0 as i64;
    // Validate KTH ID and get the user's email (should be {kth_id}@kth.se but still)
    let response = reqwest::get(format!("https://hodis.datasektionen.se/uid/{kth_id}")).await;
    if let Err(_) = response {
        ctx.say("Failed to reach authentication service.").await?;
        return Ok(());
    };

    let student = response?.json::<KTHUser>().await;
    if let Err(why) = student {
        println!("Failed to deserialise response - {}", why);
        ctx.say(format!("Couldn't find KTH ID '{kth_id}'")).await?;
        return Ok(());
    }

    let secret_code = Alphanumeric.sample_string(&mut rand::thread_rng(), 8);

    // Add to pending_auths
    sqlx::query!(
        "INSERT INTO pending_auths(discord_id, kth_id, verification_code)
            VALUES (?, ?, ?);",
        author_id,
        kth_id,
        secret_code,
    )
    .execute(&ctx.data().database)
    .await?;

    // Send an email containing a secret code...
    // Do this after adding to DB just in case smth crashes between these two points,
    // as we don't want to send an e-mail with a code that doesn't work.
    ctx.say(
        match send_authentication_email(ctx, student.unwrap(), &secret_code).await {
            Ok(_) => "Authentication e-mail sent, please check your inbox.",
            Err(_) => "Failed to send e-mail. A message containing the error has been sent to the mods for investigation.",
        },
    ).await?;

    Ok(())
}

#[poise::command(slash_command, ephemeral = true, guild_only)]
pub async fn verify(
    ctx: Context<'_>,
    #[description = "Your verification code"] code: String,
) -> Result<(), Error> {
    let author_id = ctx.author().id.0 as i64;
    let kth_id = sqlx::query!(
        "SELECT kth_id FROM pending_auths WHERE discord_id = ? AND verification_code = ?;",
        author_id,
        code
    )
    .fetch_optional(&ctx.data().database)
    .await?
    .ok_or("No pending authentication for this combination of user and verification code.")?
    .kth_id;

    let now = std::time::SystemTime::now();
    let db_time = humantime::format_rfc3339_seconds(now).to_string();

    // Add a role to the authenticated user
    let role_to_grant = decide_role_to_grant(&kth_id, ctx).await?;
    ctx.author_member()
        .await
        .unwrap()
        .to_mut()
        .add_role(ctx.http(), role_to_grant)
        .await?;

    // Insert the authentication into the DB to keep track of users -> KTH ID
    sqlx::query!(
        "INSERT INTO authenticated(discord_id, kth_id, timestamp) VALUES (?, ?, ?);",
        author_id,
        kth_id,
        db_time
    )
    .execute(&ctx.data().database)
    .await?;

    Ok(())
}

#[poise::command(
    prefix_command,
    slash_command,
    subcommands("link_role", "unlink_role", "list_phrases", "list_roles"),
    check = "check_admin",
    guild_only
)]
pub async fn passreg(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Invalid use of parent command.").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    check = "check_admin",
    ephemeral = true,
    guild_only
)]
pub async fn link_role(
    ctx: Context<'_>,
    #[description = "A passphrase"] phrase: String,
    #[description = "The role to link to the passphrase"] role: serenity::Role,
) -> Result<(), Error> {
    // Add the binding to the database
    let db_role = role.id.0 as i64;
    sqlx::query!(
        "INSERT INTO linked_roles(passphrase, role) VALUES (?, ?);",
        phrase,
        db_role
    )
    .execute(&ctx.data().database)
    .await?;

    let role_name = role.name;
    ctx.say(format!(
        "Associated role {role_name} with passphrase {phrase}."
    ))
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    check = "check_admin",
    ephemeral = true,
    guild_only
)]
pub async fn unlink_role(
    ctx: Context<'_>,
    #[description = "A passphrase"] phrase: String,
    #[description = "The role to unlink from the passphrase"] role: serenity::Role,
) -> Result<(), Error> {
    // Remove the binding from the database
    let db_role = role.id.0 as i64;
    let num_deleted = sqlx::query!(
        "DELETE FROM linked_roles WHERE passphrase = ? AND role = ?;",
        phrase,
        db_role
    )
    .execute(&ctx.data().database)
    .await?
    .rows_affected();

    let role_name = role.name;
    ctx.say(if num_deleted > 0 {
        format!("Role '{role_name}' no longer associated with passphrase '{phrase}'.")
    } else {
        format!("Role '{role_name}' not associated with phrase '{phrase}' (deleted {num_deleted} records).")
    })
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    check = "check_admin",
    ephemeral = true,
    guild_only
)]
pub async fn list_phrases(ctx: Context<'_>) -> Result<(), Error> {
    let phrases: Vec<String> = sqlx::query!("SELECT DISTINCT(passphrase) FROM linked_roles;")
        .fetch_all(&ctx.data().database)
        .await?
        .into_iter()
        .map(|res| res.passphrase)
        .collect();

    ctx.say(if phrases.is_empty() {
        String::from("No phrases currently in use.")
    } else {
        phrases.join(", ")
    })
    .await?;

    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    check = "check_admin",
    ephemeral = true,
    guild_only
)]
pub async fn list_roles(
    ctx: Context<'_>,
    #[description = "The passphrase for which to see the linked roles"] phrase: String,
) -> Result<(), Error> {
    let guild = ctx.guild().unwrap();

    let role_ids: Vec<serenity::RoleId> = sqlx::query!(
        "SELECT role FROM linked_roles WHERE passphrase = ?;",
        phrase
    )
    .fetch_all(&ctx.data().database)
    .await?
    .iter()
    .filter_map(|row| row.role.parse::<serenity::RoleId>().ok())
    .collect();

    let roles: Vec<&serenity::Role> = role_ids
        .iter()
        .filter_map(|id| guild.roles.get(&id))
        .collect();

    let role_names = roles
        .iter()
        .map(|r| r.name.clone())
        .collect::<Vec<String>>();

    ctx.say(if role_names.len() > 0 {
        role_names.join(", ")
    } else {
        format!("No roles currently associated with phrase '{phrase}'.")
    })
    .await?;
    Ok(())
}
