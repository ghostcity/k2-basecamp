extern crate base64;
extern crate hyper;
extern crate libc;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate rustc_serialize as serialize;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use hyper::Client;
use hyper::client::response::Response;
use hyper::header::{ Headers, ContentType };
use hyper::mime::{ Mime, TopLevel, SubLevel };
use hyper::status::StatusCode;
use libc::{ uint8_t, size_t };
use log::LogLevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{ Appender, Config, Logger, Root };
use serialize::base64::{ FromBase64, ToBase64, STANDARD };
use serde::Serialize;
use std::collections::HashMap;
use std::env::var;
use std::io::Read;
use std::slice;
use std::sync::{ Once, ONCE_INIT, Mutex };

const BASE_URL: &'static str = "http://localhost:8080/k2/ctapi/";

static OK: i8 = 0;
static ERR_INVALID: i8 = -1;
static ERR_HOST: i8 = -127;

static INIT: Once = ONCE_INIT;

lazy_static! {
    static ref MAP: Mutex<HashMap<u16, u16>> = Mutex::new(HashMap::new());
}

#[derive(Serialize)]
struct Empty();

#[derive(Serialize)]
struct RequestData {
    dad: u8,
    sad: u8,
    lenc: usize,
    command: String,
    lenr: usize
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct ResponseData {
    dad: u8,
    sad: u8,
    lenr: usize,
    response: String,
    responseCode: i8
}

macro_rules! post_request {
    ($path:expr) => (post_request($path, &Empty{}));
    ($path:expr, $data:expr) => (post_request($path, $data));
}

#[no_mangle]
#[allow(non_snake_case, unused_must_use)]
pub extern fn CT_init(ctn: u16, pn: u16) -> i8 {
    init_logging();

    debug!("CT_init( ctn: {}; pn: {} )", ctn, pn);

    // Do we know this CTN?
    if MAP.lock().unwrap().contains_key(&ctn) {
        return ERR_INVALID
    }

    // Build the request URL
    let endpoint = "ct_init".to_string();
    let path = endpoint + "/" + &ctn.to_string() + "/" + &pn.to_string();

    debug!("Request path: {}", path);

    // Perform the request
    let mut response = post_request!(&path);

    match response.status {
        StatusCode::Ok => {
            debug!("Response received!");

            // Cast server response
            let mut body = String::new();
            response.read_to_string(&mut body).unwrap();

            let status = body.parse::<i8>().unwrap();
            if status == OK {
                // Store CTN
                MAP.lock().unwrap().insert(ctn, pn);
            }

            debug!("Return status: {}", status);
            status
        },
        _ => {
            error!("Response not OK! Returning {}", ERR_HOST);
            debug!("Response: {:?}", response);
            ERR_HOST
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn CT_data(ctn: u16, dad: *mut uint8_t, sad: *mut uint8_t, lenc: size_t, command: *const uint8_t, lenr: *mut size_t, response: *mut uint8_t) -> i8 {
    init_logging();

    debug!("CT_data for ctn: {}", ctn);
    unsafe {
        if !MAP.lock().unwrap().contains_key(&ctn) {
            return ERR_INVALID
        }

        let dad: &mut u8 = &mut *dad;

        let sad: &mut u8 = &mut *sad;

        let lenr: &mut size_t = &mut *lenr;

        let command = slice::from_raw_parts(command, lenc as usize);

        let response = slice::from_raw_parts_mut(response, *lenr);

        let requestData = RequestData {
            dad: *dad,
            sad: *sad,
            lenc: lenc,
            command: command.to_vec().to_base64(STANDARD),
            lenr: *lenr
        };

        debug!("Additonal request parameter: dad: {}, sad: {}, lenc: {}, command: {:?}, lenr: {}, response.len(): {}", dad, sad, lenc, command, lenr, response.len());

        let pn = MAP.lock().unwrap();
        let pn = pn.get(&ctn).unwrap();
        let endpoint = "ct_data".to_string();
        let path = endpoint + "/" + &ctn.to_string() + "/" + &pn.to_string();

        debug!("Request path: {}", path);

        let mut http_response = post_request(&path, &requestData);

        match http_response.status {
            StatusCode::Ok => {
                debug!("Response received!");

                // decode server response
                let mut body = String::new();
                http_response.read_to_string(&mut body).unwrap();
                debug!("Reponse body: {}", body);

                let responseData: ResponseData = serde_json::from_str(&body).unwrap();

                if responseData.responseCode == OK {
                    debug!("dad: {}", responseData.dad);
                    *dad = responseData.dad;

                    debug!("sad: {}", responseData.sad);
                    *sad = responseData.sad;

                    debug!("lenr: {}", responseData.lenr);
                    *lenr = responseData.lenr;

                    let decoded = responseData.response.from_base64().unwrap();
                    debug!("decoded response {:?}", decoded);

                    debug!("write to given pointer");
                    for (place, element) in response.iter_mut().zip(decoded.iter()) {
                        debug!("[{}] {}", place, element);
                        *place = *element;
                    }
                }
                debug!("Return status: {}", responseData.responseCode);
                return responseData.responseCode;
            },
            _ => {
                error!("Response not OK! Returning {}", ERR_HOST);
                debug!("Response: {:?}", http_response);
                ERR_HOST
            }
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn CT_close(ctn: u16) -> i8 {
    init_logging();

    debug!("CT_close( ctn: {} )", ctn);

    // Do we know this CTN?
    if !MAP.lock().unwrap().contains_key(&ctn) {
        return ERR_INVALID
    }

    // Build the request URL
    let pn = MAP.lock().unwrap().get(&ctn).unwrap().clone();
    let endpoint = "ct_close".to_string();
    let path = endpoint + "/" + &ctn.to_string() + "/" + &pn.to_string();

    debug!("Request path: {}", path);

    // Perform the request
    let mut response = post_request!(&path);

    match response.status {
        StatusCode::Ok => {
            debug!("Response received!");

            // Cast server response
            let mut body = String::new();
            response.read_to_string(&mut body).unwrap();

            let status = body.parse::<i8>().unwrap();
            if status == OK {
                // Remove CTN
                MAP.lock().unwrap().remove(&ctn);
            }

            debug!("Return status: {}", status);
            status
        },
        _ => {
            error!("Response not OK! Returning {}", ERR_HOST);
            debug!("Response: {:?}", response);
            ERR_HOST
        }
    }
}

fn init_logging() {
   match var("K2_LOG_PATH") {
        Ok(path) => {
            INIT.call_once(|| {
                let file = FileAppender::builder()
                    .encoder(Box::new(PatternEncoder::new("{d} {l} {M}: {m}{n}")))
                    .build(path + "/" + &"k2_basecamp.log".to_string())
                    .unwrap();

                let config = Config::builder()
                    .appender(Appender::builder().build("file", Box::new(file)))
                    .logger(Logger::builder()
                        .appender("file")
                        .additive(false)
                        .build("k2_basecamp", LogLevelFilter::Debug))
                    .build(Root::builder().appender("file").build(LogLevelFilter::Error))
                    .unwrap();

                log4rs::init_config(config).unwrap();
            })
        },
        _ => ()
    }
}

fn post_request<T>(path: &str, payload: &T) -> Response
    where T: Serialize
{
    let base_url = var("K2_BASE_URL").unwrap_or(BASE_URL.to_string());
    let url = base_url + path;

    let client = Client::new();

    let mut headers = Headers::new();
    headers.set(
        ContentType(Mime(TopLevel::Application, SubLevel::Json, vec![]))
    );

    let body = serde_json::to_string(&payload).unwrap();

    return client.post(&url)
        .headers(headers)
        .body(&body[..])
        .send()
        .unwrap();
}