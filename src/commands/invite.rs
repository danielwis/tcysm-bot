use std::{env, fs};

use serenity::{framework::standard::macros::command, utils::MessageBuilder};
use serenity::framework::standard::{CommandResult, Args};
use serenity::model::prelude::*;
use serenity::prelude::*;

use serde::{Deserialize, Serialize};

use crate::{InviteTracker, InviteRoles};

/* The aim here is to...:
 * 1. Create an invite with `inv new ...`
 * 2. This triggers the "new invite" event, upon which we can add it to the db
 * 3. Associate roles with invites, e.g. `inv associate <invite-code> <role>`
 * 4. Profit
 */

#[derive(Deserialize, Serialize)]
struct Invite {
    invite_id: String,
    roles: Vec<Role>,
    uses: u32,
}

#[command]
async fn create(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // !inv create exp_age exp_uses channel
    let maxage = args.single::<u64>().unwrap_or(0);
    let maxuses = args.single::<u64>().unwrap_or(0);
    let chan = args.single::<ChannelId>().unwrap_or(ChannelId(0));
    if let Ok(invite) = chan.create_invite(ctx, |i| i.max_age(maxage).max_uses(maxuses).unique(true)).await {
        if let Err(why) = msg.channel_id.say(&ctx, format!("Created invite {code} for {chan} with a max age of {maxage} seconds, and max uses {maxuses}", code=invite.code)).await {
            println!("Error sending message to channel: {:?}", why);
        }
    } else {
        if let Err(why) = msg.channel_id.say(&ctx, format!("Error creating invite for channel {chan}")).await {
            println!("Error sending message to channel {chan}: {:?}", why);
        }
    }

    Ok(())
}

#[command]
// #[allowed_roles("mod")] // Commented out for debugging purposes
async fn link(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data_locked = {
        let data = ctx.data.read().await;
        data.get::<InviteTracker>().expect("Expected InviteTracker in data/typemap").clone()
    };

    println!("{:?}", args);

    // Check that we get the guild OK
    if let Some(guild) = msg.guild(&ctx.cache) {
        // Get one argument (the invite code) and advance the arg iterator
        match args.single_quoted::<String>() {
            Ok(invite) => {
                {
                    // Is the invite in the cache?
                    let mut invites = data_locked.write().await;
                    if let std::collections::hash_map::Entry::Occupied(mut entry) = invites.entry(invite.clone()) {
                        // Get the roles (rest of the args) and add them to the cache
                        if args.is_empty() {
                            if let Err(why) = msg.channel_id.say(&ctx, "Role arguments required: !invite <invite-code> <[roles]>").await {
                                println!("Failed to send message: {:?}", why);
                            }
                            println!("No role arguments given");
                        }

                        // We have at least one role specified
                        for arg in args.iter().quoted() {
                            let arg = arg.unwrap_or("".to_string());
                            let arg_temp = arg.clone();
                            if let Some(role) = guild.role_by_name(&arg) {
                                println!("Adding role: {:?}", role);
                                // Add the specified role to the hashmap. Does this even need extra scoping?
                                entry.get_mut().0.push(role.to_owned());
                            } else {
                                if let Err(why) = msg.channel_id.say(&ctx, "No role ".to_string() + &arg_temp + " found.").await {
                                    println!("Error sending message: {:?}", why);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                print!("An argument is required to run this command.");
            }
        }
    } else {
        println!("No guild found");
    }
    // React or show error
    Ok(())
}

#[command]
async fn unlink(_ctx: &Context, _msg: &Message) -> CommandResult {
    unimplemented!();
}

// TODO: Remove as soon as we're rid of JSON...
// Reason being that this only exists to sync the
// cached invites with the JSON file, something that
// will not need to be done after migrating to a proper
// DB where lookups happen from there, without a cache.
#[command]
async fn sync(ctx: &Context, msg: &Message) -> CommandResult {
    // Serialise the new vector and write it back to file?
    let data_locked = {
        let data_read = ctx.data.read().await;

        // Clone as the contents of data_locked otherwise go out of scope and get dropped after
        // this block
        data_read.get::<InviteTracker>().expect("Expected InviteTracker in data/TypeMap").clone()
    };

    let cached_invite_map = data_locked.read().await;

    if let Ok(db_path) = env::var("JSON_PATH") {
        let f = fs::File::create(db_path)
            .expect("Failed to create new file");
        let mut roles_to_write = Vec::<InviteRoles>::new();
        for (code, (roles, _uses)) in cached_invite_map.iter() {
            roles_to_write.push(InviteRoles { code: code.to_string(), roles: roles.to_vec() });
        }
        println!("{:?}", roles_to_write);
        serde_json::to_writer_pretty(f, &roles_to_write)
            .expect("Failed to write updated JSON");
    } else {
        if let Err(why) = msg.channel_id.say(ctx, "Could not find DB path. Ignoring...").await {
            println!("Error sending message: {:?}", why);
        }
    }

    Ok(())
}

#[command]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let data_locked = {
        let data_read = ctx.data.read().await;

        // Clone as the contents of data_locked otherwise go out of scope and get dropped after
        // this block
        data_read.get::<InviteTracker>().expect("Expected InviteTracker in data/TypeMap").clone()
    };

    let invites = data_locked.read().await;

    let mut response = MessageBuilder::new();
    response.push_bold_line("Active invites:");

    // Make an iterator out of the RwLock
    for (code, (roles, _uses)) in invites.iter() {
        response.push(code.to_string() + ": ");
        if roles.is_empty() {
            response.push_italic_line("No roles linked");
        } else {
            response.push_line(roles.iter().map(|r| r.name.to_string()).collect::<Vec<String>>().join(", "));
        }
        // println!("{:?}",invites);
    }

    if let Err(why) = msg.channel_id.say(&ctx, &response).await {
        println!("Error checking invites: {:?}", why);
    }
    Ok(())
}
