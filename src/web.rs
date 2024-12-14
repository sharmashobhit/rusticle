use std::io::Error;

use actix_web::{delete, get, post, web, App, HttpResponse, HttpServer, Responder};
use deadpool_sqlite::{Config, Manager, Pool};
use rusqlite; //::{ffi::sqlite3_auto_extension, Connection, Result};
use sqlite_vec;

// This struct represents state
#[derive(Clone)]
struct AppState {
    pool: Pool,
    // table: Mutex<VecTable<String>>, // Using `sqlite_vec` with a generic type
}

#[post("/collection")]
async fn create_collection() -> impl Responder {
    HttpResponse::Ok().body("ASD")
}

#[delete("/collection/{id}")]
async fn delete_collection() -> impl Responder {
    HttpResponse::Ok().body("ASD")
}

#[get("/")]
async fn hello(data: web::Data<AppState>) -> impl Responder {
    let conn = data.pool.get().await.unwrap();
    let result: i64 = conn
        .interact(|conn| {
            let mut stmt = conn.prepare("SELECT 1")?;
            let mut rows = stmt.query([])?;
            let row = rows.next()?.unwrap();
            row.get(0)
        })
        .await
        .unwrap()
        .unwrap();
    // let db = data.database;
    // db.execute("Select * from ASD;", "");
    HttpResponse::Ok().body("ADS")
}

#[actix_web::main]
pub async fn web_entry() -> std::io::Result<()> {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    // Configure SQLite connection pool
    let cfg = Config::new("db.sqlite3");
    let manager = Manager::from_config(&cfg, deadpool_sqlite::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();

    let conn = &pool.get().await.unwrap();
    conn.interact(|conn| {
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vector_store using vec0(key TEXT, vec float[8]);",
            (),
        )
        .unwrap();
    })
    .await
    .unwrap();

    let state = AppState { pool };
    // return Err(std::io::Error::new(std::io::ErrorKind::Other, "ASD"));
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(hello)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
