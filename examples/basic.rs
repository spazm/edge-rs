#![cfg_attr(feature = "middleware", feature(specialization))]

extern crate env_logger;
#[macro_use]
extern crate log;
extern crate edge;

#[macro_use]
extern crate lazy_static;

use edge::{json, Edge, Router, Cookie, Request, Response, Status};
use edge::header::AccessControlAllowOrigin;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use std::collections::BTreeMap;

struct MyApp {
    counter: Arc<AtomicUsize>
}

lazy_static! {
    static ref COUNTER: Arc<AtomicUsize> = Default::default();
}

impl Default for MyApp {
    fn default() -> MyApp {
        debug!("MyApp::default");
        MyApp {
            counter: COUNTER.clone()
        }
    }
}

impl MyApp {

    fn home(&mut self, _req: &Request, mut res: Response) {
        res.content_type("text/html; charset=UTF-8").header(AccessControlAllowOrigin::Any);
        res.send("<html><head><title>home</title></head><body><h1>Hello, world!</h1></body></html>")
    }

    fn hello(&mut self, req: &Request, res: Response) {
        let cnt = self.counter.fetch_add(1, Ordering::SeqCst);

        let first_name = req.param("first_name").unwrap_or("John");
        let last_name = req.param("last_name").unwrap_or("Doe");

        let mut data = BTreeMap::new();
        data.insert("first_name", json::to_value(first_name));
        data.insert("last_name", json::to_value(last_name));
        data.insert("counter", json::to_value(&cnt));
        data.insert("content", json::to_value(r#"## Contents
This is a list:

- item 1
- item 2

"#));

        res.render("hello", data)
    }

    fn settings(&mut self, req: &Request, mut res: Response) {
        let mut cookies = req.cookies();
        println!("name cookie: {}", cookies.find(|cookie| cookie.name == "name")
            .map_or("nope", |cookie| &cookie.value));

        res.content_type("text/html; charset=UTF-8");
        res.send("<html><head><title>Settings</title></head><body><h1>Settings</h1></body></html>")
    }

    fn login(&mut self, req: &Request, res: Response) {
        res.handle(|res| {
            let form = try!(req.form().map_err(|e| (Status::BadRequest, e.to_string())));
            if let Some(username) = form.get("username") {
                if username == "error" {
                    return Err((Status::BadRequest, "bad user name: error".to_string()));
                }

                let mut cookie = Cookie::new("name".to_string(), username.to_string());
                cookie.domain = Some("localhost".to_string());
                cookie.httponly = true;
                res.cookie(cookie);
            }

            Ok(Status::NoContent)
        });
    }

    fn redirect(&mut self, _req: &Request, res: Response) {
        println!("waiting 3 seconds");
        thread::sleep(Duration::from_secs(3));
        res.redirect("http://google.com", None)
    }

    fn streaming(&mut self, _req: &Request, res: Response) {
        let mut res = res.stream();
        res.append("toto".as_bytes());
        thread::sleep(Duration::from_secs(1));

        res.append("tata".as_bytes());
        thread::sleep(Duration::from_secs(1));

        res.append("titi".as_bytes());
    }

}

impl MyApp {
    fn before(&mut self, req: &mut Request) {
        println!("hello middleware for request {:?}", req.path());
    }
}

fn files(req: &Request, res: Response) {
    let path = req.path()[1..].join("/");
    res.send_file("web/".to_string() + &path)
}

fn main() {
    env_logger::init().unwrap();

    let mut edge = Edge::new("0.0.0.0:3000");
    let mut router = Router::new();
    router.get("/", MyApp::home);
    router.get("/hello/:first_name/:last_name", MyApp::hello);
    router.get("/settings", MyApp::settings);

    router.get("/redirect", MyApp::redirect);
    router.get("/streaming", MyApp::streaming);

    router.post("/login", MyApp::login);

    router.get_static("/static", files);

    // registers middleware
    router.add_middleware(MyApp::before);

    // registers view views/hello.hbs
    edge.register_template("hello");

    edge.mount("/", router);
    edge.start().unwrap();
}
