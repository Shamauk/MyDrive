use argon2::{Argon2, PasswordHash, PasswordVerifier};
use async_recursion::async_recursion;
use rocket::request::FromRequest;
use rocket::time::OffsetDateTime;
use rocket::{get, post, delete, patch, put, catch, routes, State, Request, uri, catchers};
use rocket::http::{Status, Cookie, CookieJar};
use tokio::fs::File;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::num::ParseIntError;
use std::path::{PathBuf, Path};
use std::{process::Command, net::IpAddr};
use rocket::http::{Header};
use rocket::response::{Responder, Redirect};
use rocket::response::status::NoContent;
use rocket::fs::{NamedFile};
use rocket::serde::{json::Json, Serialize, Deserialize};
use std::*;
use std::{result::Result};
use rocket::data::{ByteUnit, Data};
use rocket::request;
use std::time::Duration;
use std::time::Instant;
use std::sync::{Arc, Mutex, RwLock};
use rand::prelude::*;
use rocket::fs::FileServer;

type RateLimiterState = Arc<Mutex<RateLimiter>>;

pub struct RateLimiter {
    pub limit: u32,
    pub interval: Duration,
    pub request_count: HashMap<String, (u32, Instant)>,
}

impl RateLimiter {
    pub fn should_allow(&mut self, ip: &str) -> bool {
        let now: std::time::Instant = std::time::Instant::now();
        let entry = self.request_count.entry(ip.to_owned()).or_insert((0, now));

        if now.duration_since(entry.1) > self.interval {
            *entry = (1, now);
            true
        } else if entry.0 < self.limit {
            entry.0 += 1;
            true
        } else {
            false
        }
    }   
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self { limit: self.limit.clone(), 
            interval: self.interval.clone(), 
            request_count: self.request_count.clone() }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RateLimiter {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let client_ip = request.client_ip().map(|ip| ip.to_string()).unwrap_or_default();

        let rate_limiter_state = request.rocket().state::<RateLimiterState>().unwrap();

        match rate_limiter_state.lock() {
            Ok(mut rate_limiter_mutex) => {
                if rate_limiter_mutex.should_allow(&client_ip) {
                    request::Outcome::Success(rate_limiter_mutex.clone())
                } else {
                    request::Outcome::Failure((Status::TooManyRequests, ()))
                }
            },
            Err(_) => request::Outcome::Failure((Status::InternalServerError, ())),
        }
    }
}
        
#[derive(Responder)]
struct CustomHeaderResponder<'a, T> {
    inner: T,
    header: Header<'a>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MyAppConfig {
    directory: String,
}

#[catch(401)]
fn aunthorized_access(_req: &Request) -> Redirect {
    Redirect::to(uri!("/login"))
}

#[get("/login")]
async fn get_login(_rate_limiter: RateLimiter) -> Option<NamedFile> {
    NamedFile::open(Path::new("pages/login.html")).await.ok()
}

#[derive(Deserialize)]
pub struct User {
    pub username: String, 
    pub password: String,
}

async fn load_users_from_csv() -> Vec<User> {
    let file = std::fs::File::open("users.csv").expect("Failed to find");
    let reader = BufReader::new(file);
    let mut users = Vec::new();

    for line in reader.lines() {
        let line = line.unwrap();
        let parts : Vec<&str> = line.split("|").collect();

        if parts.len() == 2 {
            users.push(User {
                username: parts[0].to_string(),
                password: parts[1].to_string(),
            })
        }
    }

    users
}

pub struct SessionStore {
    sessions: HashMap<u64, String>,
}

type SessionStoreState = Arc<RwLock<SessionStore>>;

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            sessions: HashMap::new(),
        }
    }

    pub fn insert(&mut self, session_id: u64, username: String) {
        self.sessions.insert(session_id, username);
    }

    pub fn remove(&mut self, session_id: u64) {
        self.sessions.remove(&session_id);
    }

    pub fn get(&self, session_id: u64) -> Option<String> {
        self.sessions.get(&session_id).cloned()
    }
}

fn generate_session_id(session_store_state : &State<SessionStoreState>) -> u64 {
    match session_store_state.read() {
        Ok(session_store) => {
            let mut session_id = rand::thread_rng().gen();
            while session_store.get(session_id).is_some() {
                session_id = rand::thread_rng().gen();
            }
            session_id
        }
        Err(_) => rand::thread_rng().gen()
    }
}

pub struct AuthenticatedSession {
    pub session_id: u64,
    pub username: String,
}

fn get_session_id_from_cookie_value(cookie_value: &str) -> Result<u64, ParseIntError> {
    let session_id: u64 = cookie_value.parse()?;
    Ok(session_id)
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedSession {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let session_cookie = request.cookies().get_private("session_id");

        match session_cookie {
            Some(cookie) => {
                let session_store_state = request.rocket().state::<SessionStoreState>().unwrap();
                match session_store_state.write() {
                        Ok(session_store) => {
                            match get_session_id_from_cookie_value(cookie.value()) {
                                Ok(session_id) => {
                                    match session_store.get(session_id) {
                                        Some(username) => request::Outcome::Success(AuthenticatedSession {
                                            session_id: session_id,
                                            username: username.to_string(),
                                        }),
                                        None => request::Outcome::Failure((Status::Unauthorized, ()))
                                    }
                                },
                                Err(_) => request::Outcome::Failure((Status::Unauthorized, ()))
                            }
                            
                        },
                        Err(_) => request::Outcome::Failure((Status::InternalServerError, ())),
                }
            },
            None => request::Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}

async fn is_valid_credentials(username: String, password: String) -> bool {
    let users = load_users_from_csv().await;
    println!("num users: {}", users.len());
    for user in users {
        println!("username for a user: {}", user.username);
         if user.username.eq(&username) {
            let password_hash = PasswordHash::new(&user.password).unwrap();
            match Argon2::default().verify_password(password.as_bytes(), &password_hash) {
                Ok(_) => return true,
                Err(_) => return false,
            }
         }
    }
    false
}

#[post("/login", data = "<form>")]
async fn post_login(session: Option<AuthenticatedSession>, form: rocket::serde::json::Json<User>, 
    _rate_limiter: RateLimiter, cookies: &CookieJar<'_>, session_store_state: &State<SessionStoreState>) -> Status {
    match session {
        Some(_as) => Status::Ok,
        None => {
            let username = form.username.to_string();
            let password = form.password.to_string();

            if is_valid_credentials(username.clone(), password).await {
                let session_id = generate_session_id(session_store_state);
                
                cookies.add_private(Cookie::build("session_id", session_id.to_string())
                .path("/")
                .secure(true)
                .http_only(false)
                .expires(OffsetDateTime::checked_add(OffsetDateTime::now_utc(), rocket::time::Duration::minutes(45)))
                .finish());


                let mut session_store = session_store_state.write().unwrap();
                session_store.insert(session_id.clone(), username.clone());
                Status::Ok
            } else {
                Status::Forbidden
            }
        }
    }    
}

#[get("/logout")]
fn logout(session: AuthenticatedSession, session_store_state: &State<SessionStoreState>) -> Redirect {
    match session_store_state.write() {
        Ok(mut session_state) => {
            session_state.remove(session.session_id);
            Redirect::to("login")
        },
        Err(_) => Redirect::to("login")
    }
}

#[get("/username")]
async fn get_username(session: AuthenticatedSession) -> Result<Json<String>, Redirect> {
    Ok(Json(session.username))
}

#[get("/certificate")]
async fn get_certificate<'r>(_rate_limiter: RateLimiter) -> Result<CustomHeaderResponder<'r, NamedFile>, NoContent> {
    let result = NamedFile::open("ssl/certificate.cer").await;    

    match result {
        Ok(file) => {
            let header = Header::new("Content-Disposition", "attachment; filename=certificate.cer");
            let custom_header_responder_name_file = CustomHeaderResponder { inner: file, header: header };
            Ok(custom_header_responder_name_file)
        }
        Err(_) => Err(NoContent),
    }
}

#[derive(Serialize)]
struct SysInfo {
    used: String,
    available: String,
    total: String,
}

#[get("/sysinfo")]
async fn get_sys_info(_session: AuthenticatedSession, app_config: &State<MyAppConfig>) -> Result<Json<SysInfo>, Status> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("df -k {} | tail -n +2", &app_config.directory))
        .output()
        .expect("Failed to run shell command");

        let output_str = String::from_utf8(output.stdout).unwrap();
        let info: Vec<&str> = output_str.split_whitespace().collect();
    
        if info.len() < 4 {
            return Err(Status::InternalServerError);
        }
    
        Ok(Json(SysInfo {
            used: info[2].to_string(),
            available: info[3].to_string(),
            total: info[1].to_string(),
        }))
}

#[get("/file/<file_path..>")]
async fn get_file<'r>(session: AuthenticatedSession, file_path : PathBuf, app_config: &State<MyAppConfig>,) -> Result<CustomHeaderResponder<'r, NamedFile>, NoContent> {
    let directory = &app_config.directory;
    let user_directory = PathBuf::from(format!("{}/{}", directory, session.username));
    let mut path = user_directory.clone();
    path.push(file_path);

    if !path.starts_with(&user_directory) {
        println!("User tried to access unauthorized content");
        return Err(NoContent);
    }

    let requested_file = NamedFile::open(path.clone()).await;

    match requested_file {
        Ok(file) => {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let header = Header::new("Content-Disposition", "attachment; filename=".to_string() + file_name);
            let custom_header_responder_name_file = CustomHeaderResponder { inner: file, header: header };
            Ok(custom_header_responder_name_file)
        }
        Err(_) => Err(NoContent),
    }
}

#[delete("/file/<file_path..>")]
async fn delete_file(session: AuthenticatedSession, file_path: PathBuf, app_config: &State<MyAppConfig>) -> Status {
    let directory = &app_config.directory;
    let user_directory = PathBuf::from(format!("{}/{}", directory, session.username));
    let mut path = user_directory.clone();
    path.push(file_path.clone());

    if !path.starts_with(&user_directory) {
        println!("User tried to access unauthorized content");
        return Status::Forbidden;
    }

    if !path.exists() {
        return Status::NoContent;
    }

    if path.starts_with(format!("{}/trash", user_directory.to_str().unwrap())) {
        match tokio::fs::remove_file(path.clone()).await {
            Ok(_) => Status::Ok,
           Err(_) =>  Status::InternalServerError,
        }
    } else {
        let trash_folder = user_directory.join("trash");
        if !trash_folder.exists() {
            if let Err(_)= fs::create_dir(&trash_folder) {
                return Status::ExpectationFailed;
            }
        }

        let trash_file_path = trash_folder.join(file_path.file_name().unwrap());
        if let Err(_) = tokio::fs::rename(path.clone(), trash_file_path).await {
            return Status::NoContent;
        }

        match path.parent() {
            Some(parent_dir) => match remove_directory_if_empty(&parent_dir, &user_directory).await {
                Ok(_) => return Status::Ok,
                Err(_) => return Status::Ok,
            },
            None => return Status::Ok,
        }
    }
}

#[patch("/file/<old_file_path..>?<new_file_name>")]
async fn rename_file(session: AuthenticatedSession, old_file_path: PathBuf, new_file_name: String, app_config: &State<MyAppConfig>) -> Status {
    let directory = &app_config.directory;
    let user_directory = PathBuf::from(format!("{}/{}", directory, session.username));
    let mut old_path = user_directory.clone();
    old_path.push(old_file_path.clone());
    let mut new_path = old_path.clone();
    new_path.pop();
    new_path.push(new_file_name);

    // Ensure requested path is still within the user's directory.
    if !old_path.starts_with(&user_directory) {
        return Status::Forbidden;
    }

    // Check if the file exists
    let metadata = tokio::fs::metadata(&old_path).await;
    if metadata.is_err() || !metadata.unwrap().is_file() {
        return Status::NoContent
    }

    // Check something not already named that
    let metadata = tokio::fs::metadata(&new_path).await;
    if !metadata.is_err() {
        return Status::Conflict;
    }
   
    // Do the renaming
    if let Err(_) = tokio::fs::rename(old_path, new_path).await {
        return Status::NoContent
    }

    Status::Ok
}

#[put("/file/move/<old_file_path..>?<new_file_path..>")]
async fn move_file(session: AuthenticatedSession, old_file_path: PathBuf, new_file_path: String, app_config: &State<MyAppConfig>) -> Status {
    let directory = &app_config.directory;
    let user_directory = PathBuf::from(format!("{}/{}", directory, session.username));
    let mut old_path = user_directory.clone();
    old_path.push(old_file_path.clone());

    println!("{:?}", old_path.to_str());

    // Ensure requested path is still within the user's directory.
    if !old_path.starts_with(&user_directory) {
        return Status::Forbidden;
    }

    // Check if the file exists
    let metadata = tokio::fs::metadata(&old_path).await;
    if metadata.is_err() || !metadata.unwrap().is_file() {
        return Status::NoContent;
    }

    let mut new_path = user_directory.clone();
    new_path.push(PathBuf::from(new_file_path));

    println!("{:?}", new_path.to_str());

    // Ensure the destination path is still within the user's directory.
    if !new_path.starts_with(&user_directory) {
        return Status::Forbidden;
    }

    // Create the destination directory if it doesn't exist
    if let Some(parent) = new_path.parent() {
        if !parent.exists() {
            if let Err(_) = fs::create_dir_all(parent) {
                return Status::ExpectationFailed;
            }
        }
    }

    if let Err(_) = tokio::fs::rename(old_path.clone(), new_path).await {
        return Status::ExpectationFailed;
    }

    match old_path.parent() {
        Some(parent_dir) => match remove_directory_if_empty(&parent_dir, &user_directory).await {
            Ok(_) => return Status::Ok,
            Err(_) => return Status::Ok,
        },
        None => return Status::Ok,
    }
}

async fn remove_directory_if_empty(dir_path: &Path, root_path: &Path) -> Result<(), &'static str> {
    let mut current_dir = dir_path.to_owned();

    while current_dir != root_path {
        let entries = tokio::fs::read_dir(&current_dir).await;

        if entries.is_err() { return Err("errrr"); }

        if entries.unwrap().next_entry().await.map_err(|_| "errrr")?.is_none() {
            match tokio::fs::remove_dir(&current_dir).await {
                Ok(_) => current_dir.pop(),
                Err(_) => return Err("errrrr"),
            };
        } else {
            break;
        }
    }
    Ok(())
}

#[get("/file")]
async fn get_files(session: AuthenticatedSession, app_config: &State<MyAppConfig>) -> Result<Json<Vec<String>>, NoContent> {
    let directory = &app_config.directory;
    let user_directory = format!("{}/{}", directory, session.username);

    let all_paths = traverse_directory(&PathBuf::from(user_directory.clone()), 
        &PathBuf::from(user_directory.clone())).await;
    
    match all_paths {
        Ok(paths) => {
            let paths_str = paths
                .into_iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<String>>();
            Ok(Json(paths_str))
        }
        Err(_) => Err(NoContent),
    }
}

#[async_recursion]
async fn traverse_directory(path: &Path, base_path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut file_paths: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let file_path = entry.path();

        if metadata.is_dir() {
            let nested_files = traverse_directory(&file_path, base_path).await?;
            file_paths.extend(nested_files);
        } else if metadata.is_file() {
            let relative_path = file_path.strip_prefix(base_path).unwrap_or(&file_path);
            file_paths.push(relative_path.to_path_buf());
        }
    }

    Ok(file_paths)
}

const ILLEGAL_CHARS : [char; 14] = ['<', '>', '|', ':', '(', ')', '&', ';', '#', '?', '*','/', '\\', ' '];
fn sanitize_path(path: String) -> String {
    let mut result = path.clone();
    for char in ILLEGAL_CHARS {
        result = result.replace(char, "_");
    }
    return result;
}

pub struct FileName {
    pub name: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for FileName {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        match request.headers().get_one("X-File-Name") {
            Some(name) => request::Outcome::Success(FileName{name: sanitize_path(name.to_string())}),
            None => request::Outcome::Failure((Status::from_code(401).unwrap(), ())),
        }
    }
}

#[post("/file", data = "<file>")]
async fn post_file_from_form<'r>(session: AuthenticatedSession, 
        file_name: FileName,
        file : Data<'_>, 
        app_config: &State<MyAppConfig>, 
    ) -> Status {
    let directory : &str = &format!("{}/{}", app_config.directory, session.username);
    match tokio::fs::try_exists(directory).await {
        Ok(val) => if !val {
            match tokio::fs::create_dir(directory).await {
                Ok(_) => (),
                Err(_) => return Status::ExpectationFailed,
            }
        },
        Err(_) => return Status::ExpectationFailed,
    }

    let file_path : String = format!("{}/{}", directory, file_name.name);
    let created_file : File = match File::create(&file_path).await {
        Ok(f) => f,
        Err(_) => return Status::BadRequest,
    };

    match file.open(ByteUnit::Terabyte(1)).stream_to(created_file).await {
        Ok(_) => Status::Ok,
        Err(_) => Status::BadRequest,
    }
}

#[get("/")]
async fn index(_usr: AuthenticatedSession) -> Option<NamedFile> {
    NamedFile::open(Path::new("pages/index.html")).await.ok()
}

fn run_setup() {
    let path = Path::new("users.csv");

    if !path.exists() {
        match std::fs::File::create(path) {
            Ok(_) => (),
            Err(_) => print!("Failed to create csv"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let output = Command::new("sh")
                    .arg("-c")
                    .arg("ifconfig | grep \"inet \" | grep -v \"127.0.0.1\" | awk '{print $2}' | grep \"192\"")
                    .output()
                    .expect("Failed to run command");
    
    let local_ip_string : String = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or_default().trim().to_string();
    let local_ip = local_ip_string.parse::<IpAddr>();
    let mut figment = rocket::Config::figment().clone();
    figment = figment.merge(("my_app_config", MyAppConfig { directory: "directory".to_string()}));
    if local_ip.is_err() {
        println!("Error {}", local_ip.unwrap_err());
    } else {
        figment = figment.merge(("address", local_ip.unwrap()));
    }
    
    let app_config : MyAppConfig = figment.extract().expect("MyAppConfig");

    run_setup();

    let _ = rocket::custom(figment)
        .manage(Arc::new(Mutex::new(RateLimiter {
            limit: 10,
            interval: Duration::from_secs(60),
            request_count: HashMap::new(),
        })))
        .manage(Arc::new(RwLock::new(SessionStore::new())))
        .mount("/", FileServer::from("static"))
        .register("/", catchers![
            aunthorized_access,
        ])
        .mount(
            "/",
            routes![
                index,
                get_login,
                post_login,
                logout,
                get_certificate, 
                get_file, 
                post_file_from_form, 
                get_files,
                delete_file,
                rename_file,
                move_file,
                get_sys_info,
                get_username,
            ],
        )
        .manage(app_config)
        .launch()
        .await
        .unwrap();
    Ok(())
}