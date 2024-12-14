use log::{error, info};
use std::io::Error;

use actix_web::{
    delete, get, middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder,
};
use deadpool_sqlite::{Config, Manager, Pool};
use rusqlite; //::{ffi::sqlite3_auto_extension, Connection, Result};
use serde::Deserialize;
use sqlite_vec;

// This struct represents state
#[derive(Clone)]
struct AppState {
    pool: Pool,
    // table: Mutex<VecTable<String>>, // Using `sqlite_vec` with a generic type
}

#[derive(Deserialize)]
struct CreateCollectionRequest {
    name: String,
    vector_size: usize,
}

#[post("/collection")]
async fn create_collection(
    data: web::Data<AppState>,
    req: web::Json<CreateCollectionRequest>,
) -> impl Responder {
    info!("Creating collection: {}", req.name);
    let conn = data.pool.get().await.unwrap();
    let query = format!(
        "CREATE VIRTUAL TABLE {} using vec0(key TEXT, vec float[{}]);",
        req.name, req.vector_size
    );

    let result = conn.interact(move |conn| conn.execute(&query, ())).await;

    match result {
        Ok(_) => {
            info!("Successfully created collection: {}", req.name);
            HttpResponse::Ok().body("Collection created successfully")
        }
        Err(e) => {
            error!("Failed to create collection {}: {}", req.name, e);
            HttpResponse::InternalServerError().body("Failed to create collection")
        }
    }
}

#[delete("/collection/{name}")]
async fn delete_collection(data: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let collection_name = path.into_inner();
    let conn = data.pool.get().await.unwrap();
    let query = format!("DROP TABLE IF EXISTS {}", collection_name);

    let result = conn.interact(move |conn| conn.execute(&query, ())).await;

    match result {
        Ok(_) => HttpResponse::Ok().body("Collection deleted successfully"),
        Err(_) => HttpResponse::InternalServerError().body("Failed to delete collection"),
    }
}

#[get("/")]
async fn index(data: web::Data<AppState>) -> impl Responder {
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
    HttpResponse::Ok().body(result.to_string())
}

#[actix_web::main]
pub async fn web_entry(config: crate::config::Config) -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    info!(
        "Starting web server at {}:{}",
        config.server.host, config.server.port
    );

    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    // Configure SQLite connection pool
    let cfg = Config::new(config.database.path);
    let manager = Manager::from_config(&cfg, deadpool_sqlite::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();

    let state = AppState { pool };

    // return Err(std::io::Error::new(std::io::ErrorKind::Other, "ASD"));
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(create_collection)
            .service(delete_collection)
            .service(index)
            .wrap(Logger::default())
    })
    .bind((config.server.host, config.server.port))?
    .run()
    .await
}
