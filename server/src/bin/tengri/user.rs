//! `tengri user` — user management helpers for local/dev/test data setup.

use std::io::{self, Read};

use anyhow::{Context, anyhow};
use clap::Subcommand;
use tengri_server::user::{
    CreateUser, CreateUserPassword, CreatedUser, create_user, create_user_if_absent,
};

use super::shared::connect_pool;

#[derive(Subcommand)]
pub enum Cmd {
    /// Create a user row.
    Add {
        /// Display name (`users.name`).
        #[arg(long)]
        name: String,

        /// Optional deterministic `users.id`. Useful for fixture databases.
        #[arg(long)]
        id: Option<i32>,

        /// Login name. Compared case-insensitively at authentication time.
        #[arg(long)]
        login: Option<String>,

        /// Email address. Stored lowercased.
        #[arg(long)]
        email: Option<String>,

        /// Read a password from stdin and store it as an Argon2id hash.
        #[arg(long = "password-stdin")]
        password_stdin: bool,

        /// Raw `users.permissions` bitfield.
        #[arg(long, default_value_t = 1)]
        permissions: i32,

        /// Print the created user as JSON.
        #[arg(long)]
        json: bool,

        /// Treat an existing `id` as success.
        #[arg(long = "if-absent")]
        if_absent: bool,
    },
}

pub async fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Add {
            name,
            id,
            login,
            email,
            password_stdin,
            permissions,
            json,
            if_absent,
        } => {
            add(AddArgs {
                name,
                id,
                login,
                email,
                password_stdin,
                permissions,
                json,
                if_absent,
            })
            .await
        }
    }
}

struct AddArgs {
    name: String,
    id: Option<i32>,
    login: Option<String>,
    email: Option<String>,
    password_stdin: bool,
    permissions: i32,
    json: bool,
    if_absent: bool,
}

async fn add(args: AddArgs) -> anyhow::Result<()> {
    let password = if args.password_stdin {
        Some(CreateUserPassword::Plaintext(read_password_stdin()?))
    } else {
        None
    };

    let pool = connect_pool().await?;
    let mut input = CreateUser::internal(args.name);
    input.id = args.id;
    input.login = args.login;
    input.email = args.email;
    input.password = password;
    input.permissions = args.permissions;

    if args.if_absent {
        let Some(user) = create_user_if_absent(&pool, input).await? else {
            if args.json {
                println!("null");
            } else {
                println!("user already exists");
            }
            return Ok(());
        };
        print_user(&user, args.json)?;
        return Ok(());
    }

    let user = create_user(&pool, input).await?;
    print_user(&user, args.json)?;
    Ok(())
}

fn read_password_stdin() -> anyhow::Result<String> {
    let mut password = String::new();
    io::stdin()
        .read_to_string(&mut password)
        .context("reading password from stdin")?;
    while password.ends_with('\n') || password.ends_with('\r') {
        password.pop();
    }
    if password.is_empty() {
        return Err(anyhow!("password from stdin is empty"));
    }
    Ok(password)
}

fn print_user(user: &CreatedUser, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(user)?);
    } else {
        print_added_user(user);
    }
    Ok(())
}

fn print_added_user(user: &CreatedUser) {
    let mut detail = format!("permissions {}", user.permissions);
    if let Some(login) = &user.login {
        detail.push_str(&format!(", login {login}"));
    }
    if let Some(email) = &user.email {
        detail.push_str(&format!(", email {email}"));
    }
    println!("added user {} ({}, {detail})", user.id, user.name);
}
