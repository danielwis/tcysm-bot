use crate::checks::check_admin;
use crate::{Context, Error};
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

#[poise::command(slash_command, prefix_command, ephemeral = true)]
pub async fn passphrase(
    ctx: Context<'_>,
    #[description = "The passphrase"] passphrase: String,
) -> Result<(), Error> {
    let existing_phrase = ctx.data().open_reg_phrase.lock().unwrap().clone();

    if let Some(phrase) = existing_phrase {
        if phrase == passphrase {
            ctx.author_member()
                .await
                .unwrap()
                .to_mut()
                .add_role(&ctx.serenity_context().http, ctx.data().student_role_id)
                .await?;

            ctx.say("Successfully authenticated as student.").await?;
        } else {
            ctx.say("Incorrect passphrase. Please try again.").await?;
        }
    } else {
        ctx.say("Passphrase authentication is not currently enabled. If you believe this to be a mistake, please contact a moderator.").await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command, ephemeral = true)]
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
            "authorised" => {
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
        "INSERT INTO auths(user_id, role, status) VALUES (?, ?, ?);",
        author_id,
        role_to_assign,
        "pending"
    )
    .execute(&ctx.data().database)
    .await?;

    Ok(())
}

#[poise::command(
    prefix_command,
    slash_command,
    subcommands("open", "close", "check"),
    check = "check_admin"
)]
pub async fn passreg(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Invalid use of parent command.").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_admin", ephemeral = true)]
pub async fn open(
    ctx: Context<'_>,
    #[description = "The passphrase to use for authentication"] phrase: String,
) -> Result<(), Error> {
    // Don't use `if let` to avoid holding lock across the `await` below.
    let existing = ctx.data().open_reg_phrase.lock().unwrap().clone();
    if let Some(existing_phrase) = existing {
        ctx.say(format!("Registration already open with phrase {existing_phrase}. Close and re-open to change the phrase."))
            .await?;
    } else {
        *ctx.data().open_reg_phrase.lock().unwrap() = Some(phrase.clone());
        ctx.say(format!("Passphrase registration opened with '{phrase}'"))
            .await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_admin", ephemeral = true)]
pub async fn close(ctx: Context<'_>) -> Result<(), Error> {
    // Don't use `if let` to avoid holding lock across the `await` below.
    let existing = ctx.data().open_reg_phrase.lock().unwrap().clone();
    if let Some(existing_phrase) = existing {
        ctx.say(format!(
            "Closing open registration with '{existing_phrase}'."
        ))
        .await?;
    }
    *ctx.data().open_reg_phrase.lock().unwrap() = None;

    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_admin", ephemeral = true)]
pub async fn check(ctx: Context<'_>) -> Result<(), Error> {
    // Don't use `if let` to avoid holding lock across the `await` below.
    let existing = ctx.data().open_reg_phrase.lock().unwrap().clone();
    if let Some(existing_phrase) = existing {
        ctx.say(format!("Registration open. Phrase: '{existing_phrase}'."))
            .await?;
    } else {
        ctx.say("Closed.").await?;
    }

    Ok(())
}
