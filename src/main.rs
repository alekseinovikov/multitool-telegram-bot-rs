use dotenv::dotenv;
use sqlx::{Connection, SqliteConnection, SqlitePool};
use std::env;
use sqlx::migrate::MigrateDatabase;
use teloxide::dispatching::dialogue::serializer::Json;
use teloxide::dispatching::dialogue::{ErasedStorage, SqliteStorage, Storage};
use teloxide::dispatching::{HandlerExt, UpdateFilterExt};
use teloxide::prelude::{Dialogue, Dispatcher, Message, Requester, Update};
use teloxide::{dptree, Bot};

type WelcomeDialogue = Dialogue<State, ErasedStorage<State>>;
type SqliteLocalStorage = std::sync::Arc<ErasedStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
enum State {
    #[default]
    Start,
    AwaitingUserName,
    ReceivedUserName {
        user_name: String,
    },
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    let storage_file_path =
        env::var("SQLITE_DB_PATH").map_or("storage.db".to_string(), |path| path);

    if !sqlx::Sqlite::database_exists(&storage_file_path).await.expect("Failed to check if SQLite database exists") {
        sqlx::Sqlite::create_database(&storage_file_path).await.expect("Failed to create SQLite database");
    }

    let sqlite: SqlitePool = SqlitePool::connect(storage_file_path.as_str()).await.expect("Failed to connect to SQLite");
    let storage: SqliteLocalStorage = SqliteStorage::open(&storage_file_path, Json)
        .await
        .expect("Failed to open SQLite storage")
        .erase();

    let bot = Bot::from_env();

    log::info!("Launching bot...");

    let handler = Update::filter_message()
        .enter_dialogue::<Message, ErasedStorage<State>, State>()
        .branch(dptree::case![State::Start].endpoint(start))
        .branch(dptree::case![State::AwaitingUserName].endpoint(receive_user_name));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, sqlite])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn start(bot: Bot, dialogue: WelcomeDialogue, msg: Message, sqlite: SqlitePool) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        "Давай начнем! Скажи мне, как мне тебя называть?",
    )
    .await?;
    dialogue.update(State::AwaitingUserName).await?;
    Ok(())
}

async fn receive_user_name(bot: Bot, dialogue: WelcomeDialogue, msg: Message, sqlite: SqlitePool) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            dialogue.update(State::ReceivedUserName{user_name: text.to_string()}).await?;
            bot.send_message(msg.chat.id, format!("Приятно познакомиться, {}!", text)).await?;
            // TODO: Continue the dialogue
        }
        None => {
            bot.send_message(msg.chat.id, "Пришли мне обычный текст!").await?;
        }
    }

    Ok(())
}
