#![feature(async_await)]

use sqlx::{postgres::Connection, ConnectOptions};
use std::io;

// TODO: ToSql and FromSql (to [de]serialize values from/to Rust and SQL)
// TODO: Connection strings ala postgres@localhost/sqlx_dev

#[runtime::main(runtime_tokio::Tokio)]
async fn main() -> io::Result<()> {
    env_logger::init();

    // Connect as postgres / postgres and DROP the sqlx__dev database
    // if exists and then re-create it
    let mut conn = Connection::establish(
        ConnectOptions::new()
            .host("127.0.0.1")
            .port(5432)
            .user("postgres")
            .database("postgres"),
    )
    .await?;

    // println!(" :: drop database (if exists) sqlx__dev");

    // conn.prepare("DROP DATABASE IF EXISTS sqlx__dev")
    //     .execute()
    //     .await?;

    // println!(" :: create database sqlx__dev");

    // conn.prepare("CREATE DATABASE sqlx__dev").execute().await?;

    // conn.close().await?;

    // let mut conn = Connection::establish(
    //     ConnectOptions::new()
    //         .host("127.0.0.1")
    //         .port(5432)
    //         .user("postgres")
    //         .database("sqlx__dev"),
    // )
    // .await?;

    //     println!(" :: create schema");

    //     conn.prepare(
    //         r#"
    // CREATE TABLE IF NOT EXISTS users (
    //     id BIGSERIAL PRIMARY KEY,
    //     name TEXT NOT NULL
    // );
    //         "#,
    //     )
    //     .execute()
    //     .await?;

    //     println!(" :: insert");

    let row = conn
        .prepare("SELECT pg_typeof($1), pg_typeof($2)")
        .bind(20)
        .bind_as::<sqlx::postgres::types::BigInt, _>(10)
        .get()
        .await?;

    println!("{:?}", row);

    // println!(" :: select");

    // conn.prepare("SELECT id FROM users")
    //     .select()
    //     .try_for_each(|row| {
    //         let id = row.get(0);

    //         println!("select {:?}", id);

    //         future::ok(())
    //     })
    //     .await?;

    conn.close().await?;

    Ok(())
}
