use log::{error, info};

use actix_web::{
    delete, get, middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder,
};
use deadpool_sqlite::{Config, Manager, Pool};
use rusqlite; //::{ffi::sqlite3_auto_extension, Connection, Result};
use serde::{Deserialize, Serialize};
use sqlite_vec;
use zerocopy::IntoBytes;

// This struct represents state
#[derive(Clone)]
struct AppState {
    pool: Pool,
    config: crate::config::Config,
    // table: Mutex<VecTable<String>>, // Using `sqlite_vec` with a generic type
}

#[derive(Deserialize, Serialize)]
struct CreateCollectionRequest {
    name: String,
    vector_size: usize,
}

#[derive(Deserialize, Serialize)]
struct CreateVectorRequest {
    text: String,
}

#[derive(Deserialize, Serialize)]
struct SearchRequest {
    text: String,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResult {
    rowid: i64,
    key: String,
    similarity: f32,
}

#[post("/collection")]
async fn create_collection(
    data: web::Data<AppState>,
    req: web::Json<CreateCollectionRequest>,
) -> impl Responder {
    info!("Creating collection: {}", req.name);
    let conn = data.pool.get().await.unwrap();
    let query = format!(
        "CREATE VIRTUAL TABLE {} using vec0(key TEXT, vec float[{}] distance_metric=cosine);",
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

#[post("/collection/{name}")]
async fn insert_vector(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: web::Json<CreateVectorRequest>,
) -> impl Responder {
    let collection_name = path.into_inner();

    // Generate embedding
    match crate::embedding::embed(&data.config, vec![&req.text]).await {
        Ok(vector) => {
            let conn = data.pool.get().await.unwrap();

            let result = conn
                .interact(move |conn| {
                    let tx = conn.transaction().unwrap();
                    {
                        let mut query = tx
                            .prepare(
                                format!(
                                    "INSERT INTO {} (key, vec) VALUES (?, ?)",
                                    collection_name.as_str()
                                )
                                .as_str(),
                            )
                            .unwrap();
                        for vector in &vector {
                            query
                                .execute(rusqlite::params![&req.text, vector.as_bytes()])
                                .unwrap();
                        }
                    }
                    tx.commit().unwrap();
                })
                .await;

            match result {
                Ok(_) => HttpResponse::Ok().body("Vector inserted successfully"),
                Err(e) => {
                    error!("Failed to insert vector: {}", e);
                    HttpResponse::InternalServerError().body("Failed to insert vector")
                }
            }
        }
        Err(e) => {
            error!("Failed to generate embedding: {}", e);
            HttpResponse::InternalServerError().body("Failed to generate embedding")
        }
    }
}

#[post("/collection/{name}/search")]
async fn search_vectors(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: web::Json<SearchRequest>,
) -> impl Responder {
    let collection_name = path.into_inner();
    let limit = req.limit.unwrap_or(10);

    // Generate embedding for search text
    match crate::embedding::embed(&data.config, vec![&req.text]).await {
        Ok(vector) => {
            // dbg!(&vector[0].as_str());
            let conn = data.pool.get().await.unwrap();
            let query = format!(
                "SELECT rowid, key, distance FROM {} WHERE vec MATCH ?1 ORDER BY distance LIMIT {}",
                collection_name, limit
            );

            let result = conn
                .interact(move |conn| {
                    let mut stmt = conn.prepare(query.as_str())?;
                    let rows = stmt.query_map([&vector[0].as_bytes()], |row| {
                        Ok(SearchResult {
                            rowid: row.get(0)?,
                            key: row.get(1)?,
                            similarity: 1.0 - row.get::<_, f32>(2)?,
                        })
                    })?;

                    let mut results = Vec::new();
                    for row in rows {
                        results.push(row?);
                    }
                    Ok::<Vec<SearchResult>, rusqlite::Error>(results)
                })
                .await;

            match result {
                Ok(Ok(results)) => HttpResponse::Ok().json(results),
                Ok(Err(e)) => {
                    dbg!(&e);
                    error!("Database error during search: {}", e);
                    HttpResponse::InternalServerError().body("Search failed")
                }
                Err(e) => {
                    error!("Pool error during search: {}", e);
                    HttpResponse::InternalServerError().body("Search failed")
                }
            }
        }
        Err(e) => {
            error!("Failed to generate embedding: {}", e);
            HttpResponse::InternalServerError().body("Failed to generate embedding")
        }
    }
}

#[get("/")]
async fn index(data: web::Data<AppState>) -> impl Responder {
    let conn = data.pool.get().await.unwrap();
    let result: u8 = conn
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
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    // Configure SQLite connection pool
    let cfg = Config::new(&config.database.path);
    let manager = Manager::from_config(&cfg, deadpool_sqlite::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();
    let state = AppState {
        pool,
        config: config.clone(),
    };
    info!(
        "Starting web server at {}:{}",
        config.server.host, config.server.port
    );

    // return Err(std::io::Error::new(std::io::ErrorKind::Other, "ASD"));
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(create_collection)
            .service(delete_collection)
            .service(insert_vector)
            .service(search_vectors) // Add the new handler
            .service(index)
            .wrap(Logger::default())
    })
    .bind((config.server.host, config.server.port))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test};

    async fn create_test_app() -> (
        web::Data<AppState>,
        App<
            impl actix_web::dev::ServiceFactory<
                actix_web::dev::ServiceRequest,
                Config = (),
                Response = actix_web::dev::ServiceResponse,
                Error = actix_web::Error,
                InitError = (),
            >,
        >,
    ) {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let app_config = crate::config::Config::default();
        let cfg = Config::new(":memory:");
        let manager = Manager::from_config(&cfg, deadpool_sqlite::Runtime::Tokio1);
        let pool = Pool::builder(manager).build().unwrap();
        let state = AppState {
            pool,
            config: app_config,
        };
        let app_data = web::Data::new(state);

        let app = App::new()
            .app_data(app_data.clone())
            .service(index)
            .service(create_collection)
            .service(delete_collection)
            .service(insert_vector)
            .service(search_vectors);

        (app_data, app)
    }

    #[actix_web::test]
    async fn test_index() {
        let (_, app) = create_test_app().await;
        let app = test::init_service(app).await;
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp).await;
        assert_eq!(body, "1");
    }

    #[actix_web::test]
    async fn test_create_collection() {
        let (app_data, app) = create_test_app().await;
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/collection")
            .set_json(&CreateCollectionRequest {
                name: "test".to_string(),
                vector_size: 768,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        // Check if table exists and verify its structure
        let conn = app_data.pool.get().await.unwrap();
        let table_info = conn
            .interact(|conn| {
                conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='test'")?
                    .query_row([], |row| row.get::<_, String>(0))
            })
            .await
            .unwrap()
            .unwrap();

        assert!(table_info.contains("vec float[768]"));
        assert!(table_info.contains("key TEXT"));
    }

    #[actix_web::test]
    async fn test_delete_collection() {
        let (app_data, app) = create_test_app().await;
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/collection")
            .set_json(&CreateCollectionRequest {
                name: "test".to_string(),
                vector_size: 10,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::delete()
            .uri("/collection/test")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        // Check if table exists
        let conn = app_data.pool.get().await.unwrap();
        let table_info = conn
            .interact(|conn| {
                conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='test'")?
                    .query_row([], |row| row.get::<_, String>(0))
            })
            .await
            .unwrap();

        assert!(table_info.is_err());
    }

    #[actix_web::test]
    async fn test_insert_vector() {
        let (app_data, app) = create_test_app().await;
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/collection")
            .set_json(&CreateCollectionRequest {
                name: "test".to_string(),
                vector_size: 768,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::post()
            .uri("/collection/test")
            .set_json(&CreateVectorRequest {
                text: "test".to_string(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        // Check if vector was inserted
        let conn = app_data.pool.get().await.unwrap();
        let vector = conn
            .interact(|conn| {
                conn.prepare("SELECT vec FROM test WHERE key = 'test'")?
                    .query_row([], |row| row.get::<_, Vec<u8>>(0))
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(vector.len(), 768 * 4);
    }

    #[actix_web::test]
    async fn test_search_vectors() {
        let (app_data, app) = create_test_app().await;
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/collection")
            .set_json(&CreateCollectionRequest {
                name: "test".to_string(),
                vector_size: 768,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::post()
            .uri("/collection/test")
            .set_json(&CreateVectorRequest {
                text: "Cricket legend Sachin tendulkar".to_string(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::post()
            .uri("/collection/test/search")
            .set_json(&SearchRequest {
                text: "Roger Federer is a great tennis player".to_string(),
                limit: Some(1),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        // let body = test::read_body(resp).await;
        // assert_eq!(body, "[]");
    }
}
