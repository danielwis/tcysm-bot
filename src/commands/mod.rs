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

#[poise::command(slash_command, prefix_command)]
pub async fn authenticate(
    ctx: Context<'_>,
    #[description = "Your KTH ID"] kth_id: String,
) -> Result<(), Error> {
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

    if get_employee_uids().await?.contains(&kth_id) {
        ctx.say(format!("Sending e-mail to {email} (teacher role)"))
            .await?;
    } else {
        ctx.say(format!("Sending e-mail to {email} (student role)")).await?;
    }

    Ok(())
}
