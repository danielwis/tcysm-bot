use crate::checks::check_admin;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use reqwest;
use scraper::{Html, Selector};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Student {
    // uid: String,
    mail: String,
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

    if matching_roles.len() == 0 {
        ctx.say("This phrase does not currently seem to be linked to any roles. Please try again.")
            .await?;
        return Ok(());
    }

    let author_id = ctx.author().id.0 as i64;

    let now = std::time::SystemTime::now();
    let db_time = humantime::format_rfc3339_seconds(now).to_string();
    for role in &matching_roles {
        // Add auth to DB
        let db_role = role.0.to_string();
        sqlx::query!(
        "INSERT OR IGNORE INTO auths(user_id, role, status, passphrase, auth_type, kth_id, authenticated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?);",
        author_id,
        db_role,
        "authenticated",
        passphrase,
        "passphrase",
        None::<String>,
        db_time
    )
        .execute(&mut *transaction)
        .await?.rows_affected();

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

#[poise::command(slash_command, prefix_command, ephemeral = true, guild_only)]
pub async fn id(
    ctx: Context<'_>,
    #[description = "Your KTH ID"] kth_id: String,
) -> Result<(), Error> {
    let author_id = ctx.author().id.0 as i64;
    if let Some(auth_status) = sqlx::query!("SELECT status FROM auths WHERE user_id = ?", author_id)
        .fetch_optional(&ctx.data().database)
        .await?
    {
        match auth_status.status.as_str() {
            "pending" => {
                ctx.say("Authentication already initiated. Please check your e-mail")
                    .await?;
            }
            "authenticated" => {
                ctx.say("Already authorised").await?;
            }
            _ => {
                ctx.say("What?").await?;
            }
        }
        return Ok(());
    }

    // Discord user not already authenticated. TODO check kth ID too?
    // TODO: Store passphrase too for verification lmao
    let response = reqwest::get(format!("https://hodis.datasektionen.se/uid/{kth_id}")).await;
    if let Err(_) = response {
        ctx.say("Failed to reach authentication service.").await?;
        return Ok(());
    };

    let student = response?.json::<Student>().await;
    if let Err(why) = student {
        println!("Failed to deserialise response - {}", why);
        ctx.say(format!("Couldn't find KTH ID '{kth_id}'")).await?;
        return Ok(());
    }

    let email = student?.mail.clone();

    let role_to_assign = if get_employee_uids().await?.contains(&kth_id) {
        ctx.say(format!("Sending e-mail to {email} (teacher role)"))
            .await?;
        String::from("Student")
    } else {
        ctx.say(format!("Sending e-mail to {email} (student role)"))
            .await?;
        String::from("Teacher")
    };

    sqlx::query!(
        "INSERT INTO auths(user_id, role, status, passphrase, auth_type, kth_id)
            VALUES (?, ?, ?, ?, ?, ?);",
        author_id,
        role_to_assign,
        "pending",
        "TODO",
        "kth_id",
        kth_id
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

    ctx.say(if phrases.len() > 0 {
        phrases.join(", ")
    } else {
        String::from("No phrases currently in use.")
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
