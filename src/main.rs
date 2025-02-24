use std::{env, sync::RwLock};

use regex::Regex;
use salvo::{http::HeaderMap, prelude::*};
use tracing::{error, info};

const ENV_PREFIX: &str = "SR_REDIR";

const REDIRECT_HTML_PAGE: &str = r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={REDIRECT_URL}"><title>Redirecting...</title></head><body><p>If you are not redirected, <a href="{REDIRECT_URL}">click here</a>.</p></body></html>"#;

#[derive(Debug)]
enum ParseError {
    Missing(String),
    WrongFormat(String, String),
}

impl ParseError {
    fn unpack(self) {
        match self {
            Self::Missing(d) => {
                error!("Variable \"{d}\" is missing! Exiting.");
            }
            Self::WrongFormat(key, expected_type) => {
                error!("Variable \"{key}\" has wrong type, expected {expected_type}! Exiting.");
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RedirEntry {
    paths: Vec<String>,
    target: String,
    code: StatusCode,
    js_only: bool,
    preserve_params: bool,
}

impl RedirEntry {
    fn from_vars(name: &str) -> Result<RedirEntry, ParseError> {
        let paths_key = format!("{ENV_PREFIX}_{name}");
        let paths: Vec<String> = match env::var(&paths_key) {
            Ok(d) => d
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
            Err(_) => return Err(ParseError::Missing(paths_key)),
        };
        let target_key = format!("{ENV_PREFIX}_{name}__TARGET");
        let target = match env::var(&target_key) {
            Ok(d) => d,
            Err(_) => return Err(ParseError::Missing(target_key)),
        };
        let code_key = format!("{ENV_PREFIX}_{name}__CODE");
        let code = match env::var(&code_key) {
            Ok(d) => match d.parse::<u16>() {
                Ok(d) => match StatusCode::from_u16(d) {
                    Ok(d) => d,
                    Err(_) => return Err(ParseError::WrongFormat(code_key, "Integer".to_string())),
                },
                Err(_) => return Err(ParseError::WrongFormat(code_key, "Integer".to_string())),
            },
            Err(_) => return Err(ParseError::Missing(code_key)),
        };
        let js_only_key = format!("{ENV_PREFIX}_{name}__JS_ONLY");
        let js_only = match env::var(&js_only_key) {
            Ok(d) => match d.parse::<bool>() {
                Ok(d) => d,
                Err(_) => return Err(ParseError::WrongFormat(js_only_key, "Boolean".to_string())),
            },
            Err(_) => false,
        };
        let preserve_params_key = format!("{ENV_PREFIX}_{name}__PRESERVE_PARAMS");
        let preserve_params = match env::var(&preserve_params_key) {
            Ok(d) => match d.parse::<bool>() {
                Ok(d) => d,
                Err(_) => {
                    return Err(ParseError::WrongFormat(
                        preserve_params_key,
                        "Boolean".to_string(),
                    ))
                }
            },
            Err(_) => false,
        };
        Ok(RedirEntry {
            paths,
            target,
            code,
            js_only,
            preserve_params,
        })
    }

    fn extract_names() -> Vec<String> {
        let re = Regex::new(&format!(r"^{ENV_PREFIX}_([a-zA-Z0-9]+)$")).unwrap();
        let mut names: Vec<String> = vec![];
        for (key, _) in env::vars() {
            if re.find_iter(&key).next().is_some() {
                // This will return the captured groups
                let caps = re.captures(&key).unwrap(); // If we have a match, use captures to get the group
                names.push(caps[1].to_string()); // As_str converts &str to String
            }
        }
        names
    }
    /*
    fn get_map() -> Result<HashMap<String, RedirEntry>, ParseError> {
        let names = RedirEntry::extract_names();
        let mut map: HashMap<String, RedirEntry> = HashMap::new();
        for name in names {
            info!("Found handler: {}", &name);
            map.insert(name.clone(), RedirEntry::from_vars(&name)?);
        }
        Ok(map)
    }
    */
    fn get_routers() -> Result<Vec<Router>, ParseError> {
        let names: Vec<String> = RedirEntry::extract_names();
        info!("Names found: {:?}", &names);
        let mut routers: Vec<Router> = vec![];
        for name in names {
            info!("Found handler: {}", &name);
            let entry = RedirEntry::from_vars(&name)?;
            for path in entry.clone().paths {
                info!("Handler registered for {}", &path);
                routers.push(Router::with_path(path).get(RedirEntryHandler {
                    entry: entry.clone().into(),
                }));
            }
        }
        Ok(routers)
    }
}

pub struct RedirEntryHandler {
    entry: RwLock<RedirEntry>,
}

#[async_trait]
impl Handler for RedirEntryHandler {
    async fn handle(
        &self,
        req: &mut Request,
        _depot: &mut Depot,
        res: &mut Response,
        _ctrl: &mut FlowCtrl,
    ) {
        let entry = self.entry.read().unwrap();
        let params: String = if entry.preserve_params {
            req.uri().query().unwrap_or_default().to_string()
        } else {
            "".to_string()
        };
        let target = if params.is_empty() {
            entry.target.to_string()
        } else {
            format!("{}?{}", entry.target, params)
        };
        let mut headers = HeaderMap::new();
        if entry.js_only {
            let page = REDIRECT_HTML_PAGE.replace("{REDIRECT_URL}", &target);
            headers.append("Content-Type", "text/html".parse().unwrap());
            res.status_code(StatusCode::OK);
            res.set_headers(headers);
            res.render(Text::Html(page));
            return;
        } else {
            headers.append(
                "Location",
                target.parse().unwrap(),
            );
            res.set_headers(headers);
            res.status_code(entry.code);
            return;
        }
    }
}
#[handler]
async fn error_handler(res: &mut Response) {
    res.status_code(StatusCode::NOT_FOUND);
    res.render("");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let routers = match RedirEntry::get_routers() {
        Ok(d) => d,
        Err(e) => {
            e.unpack();
            return;
        }
    };
    let mut router = Router::new();
    for redir_router in routers.into_iter() {
        router = router.push(redir_router);
    }
    router = router.push(Router::new().goal(error_handler));
    let interface = env::var(format!("{ENV_PREFIX}__HOST")).unwrap_or("0.0.0.0:8080".to_string());
    let acceptor = TcpListener::new(interface).bind().await;
    Server::new(acceptor).serve(router).await;
}
