//! Edge is a Web framework that is simple to use, with the most common things
//! you need out of the box, and flexible, supporting both synchronous and asynchronous
//! request handling styles; see below for examples.
//!
//! The crate exports the things that you often need from dependencies, such as headers (from `hyper`),
//! cookies (from `cookie`) and JSON serialization (from `serde_json`).
//!
//! Please note that this is an early version, and the API is likely to evolve.
//!
//! ## Overview
//!
//! In Edge you must define an *application structure* that contains the state of your application.
//! You instantiate a container around this application, and associate GET/POST/... requests
//! with given URLs to methods of your application. The container handles the routing and
//! delegates calls to the appropriate methods.
//!
//! Note that the state cannot be mutated, as is usual in Rust (and enforced by the underlying HTTP server
//! this crate uses, a.k.a. Hyper). Use appropriate concurrent data structures if you need
//! shared mutable variables: locks, mutexes, channels, etc.
//!
//! ## Why another Web framework in Rust?
//!
//! Because I wanted a simple Web framework with:
//!
//!   1. everything I needed out of the box, like cookies and forms and templating, without having to dig up third-party crates,
//!   1. the possibility to describe my application as a struct, so that callbacks could use a state (even if just for configuration).
//!
//! We focus on integration rather than modularity.
//! I hope you like this crate, if it misses something to fit your needs just open an issue or make a pull request!
//!
//! And please keep in mind that the framework is in a (very) early stage :-)
//!
//! ## Hello World
//!
//! The most basic application: no state, a single page that prints Hello, world!
//!
//! ```no_run
//! extern crate edge;
//!
//! use edge::{Edge, Request, Response};
//!
//! struct Hello;
//! impl Hello {
//!     fn hello(&self, _req: &mut Request, mut res: Response) {
//!         res.content_type("text/plain");
//!         res.send("Hello, world!")
//!     }
//! }
//!
//! fn main() {
//!     let mut edge = Edge::new("0.0.0.0:3000", Hello);
//!     edge.get("/", Hello::hello);
//!     edge.start().unwrap();
//! }
//! ```
//!
//! ## Asynchronous handling
//!
//! Under the hood, Edge uses the asynchronous version of Hyper. This means that to get the maximum
//! performance, you should avoid waiting in a handler, so that other requests
//! can be served as soon as possible. In that example, the handler waits in a separate thread before sending
//! the response.
//!
//! ```no_run
//! extern crate edge;
//!
//! use edge::{Edge, Request, Response};
//! use std::thread;
//! use std::time::Duration;
//!
//! struct AsyncHello;
//! impl AsyncHello {
//!     fn hello(&self, _req: &mut Request, mut res: Response) {
//!         thread::spawn(move || {
//!             println!("waiting 1 second");
//!             thread::sleep(Duration::from_secs(1));
//!
//!             res.content_type("text/plain");
//!             res.send("Hello, world!")
//!         });
//!
//!         // the handler returns immediately without waiting for the thread
//!     }
//! }
//!
//! fn main() {
//!     let mut edge = Edge::new("0.0.0.0:3000", AsyncHello);
//!     edge.get("/", AsyncHello::hello);
//!     edge.start().unwrap();
//! }
//! ```
//!
//! ## Templating
//!
//! Here our application has a version, still a single handler except this time
//! it accepts any page name, and renders a Handlebars template.  We're also
//! setting a custom Server header.
//!
//! ```no_run
//! extern crate edge;
//!
//! use edge::{Edge, Request, Response, Status};
//! use edge::header::Server;
//! use std::collections::BTreeMap;
//!
//! struct Templating {
//!     version: &'static str
//! }
//!
//! impl Templating {
//!     fn page_handler(&self, req: &mut Request, mut res: Response) {
//!         let mut data = BTreeMap::new();
//!         data.insert("title", req.param("page").unwrap());
//!         data.insert("version", self.version);
//!
//!         res.content_type("text/html").header(Server(format!("Edge version {}", self.version)));
//!         res.render("tmpl", data)
//!     }
//! }
//!
//! fn main() {
//!     let app = Templating { version: "0.1" };
//!     let mut edge = Edge::new("0.0.0.0:3000", app);
//!     edge.get("/:page", Templating::page_handler);
//!     edge.register_template("tmpl");
//!     edge.start().unwrap();
//! }
//! ```
//!
//! ## Using a shared mutable counter
//!
//! In this example, we use an atomic integer to track a counter. This shows a very basic
//! kind of shared state for a handler. In practice, it's best to avoid using blocking
//! mechanisms (locks, mutexes) in a handler directly. Prefer non-blocking calls,
//! like channels' try_recv, or move blocking code in a separate thread,
//! see the example for asynchronous handling above.
//!
//! ```no_run
//! extern crate edge;
//!
//! use edge::{Edge, Request, Response, Status};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! struct Counting {
//!     counter: AtomicUsize
//! }
//!
//! impl Counting {
//!     fn new() -> Counting { Counting { counter: AtomicUsize::new(0) } }
//!
//!     fn home(&self, _req: &mut Request, mut res: Response) {
//!         let visits = self.counter.load(Ordering::Relaxed);
//!         self.counter.store(visits + 1, Ordering::Relaxed);
//!
//!         res.status(Status::Ok).content_type("text/plain");
//!         res.send(format!("Hello, world! {} visits", visits))
//!     }
//! }
//!
//! fn main() {
//!     let mut cter = Edge::new("0.0.0.0:3000", Counting::new());
//!     cter.get("/", Counting::home);
//!     cter.start().unwrap();
//! }
//! ```

#![cfg_attr(feature = "middleware", feature(specialization))]

extern crate crossbeam;
extern crate handlebars;
extern crate hyper;
extern crate num_cpus;
extern crate pulldown_cmark;
extern crate scoped_pool;
extern crate serde;
extern crate serde_json;
extern crate url;

#[macro_use]
extern crate log;

pub use hyper::header as header;
pub use header::CookiePair as Cookie;
pub use hyper::status::StatusCode as Status;

pub use serde_json::value as value;

use handlebars::{Context, Handlebars, Helper, RenderContext, RenderError};

use hyper::Method;
use hyper::method::Method::{Delete, Get, Head, Post, Put};
use hyper::net::HttpListener;
use hyper::server::Server;

use pulldown_cmark::Parser;
use pulldown_cmark::{Options, OPTION_ENABLE_TABLES, OPTION_ENABLE_FOOTNOTES};
use pulldown_cmark::html;

use scoped_pool::Pool;

use std::fs::read_dir;
use std::io::Result as IoResult;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};

mod buffer;
mod client;
mod handler;
mod router;
mod request;
mod response;

pub use client::Client;
pub use request::Request;
pub use response::{Response, Streaming};
pub use router::{Callback, Middleware};

use router::{Router, Instance, Static};

/// Structure for an Edge application.
pub struct Edge<T> {
    router: Router<T>,
    handlebars: Handlebars
}

#[cfg(feature = "middleware")]
/// Default middleware implementation (if using specialization)
impl<T> Middleware for T {
    default fn before(&mut self, _: &mut Request) {
    }
}

#[cfg(not(feature = "middleware"))]
/// Default middleware implementation (if using specialization)
impl<T> Middleware for T {
    fn before(&mut self, _: &mut Request) {
    }
}

impl<T> Edge<T> {

    /// Creates an Edge application using the given address and application.
    pub fn new(addr: &str) -> Edge<T> {
        let mut handlebars = Handlebars::new();
        init_handlebars(&mut handlebars).unwrap();

        Edge {
            router: Router::new(addr),
            handlebars: handlebars
        }
    }

    /// Registers a callback for the given path for GET requests.
    pub fn get(&mut self, path: &str, callback: Instance<T>) {
        self.insert(Get, path, callback);
    }

    /// Registers a callback for the given path for POST requests.
    pub fn post(&mut self, path: &str, callback: Instance<T>) {
        self.insert(Post, path, callback);
    }

    /// Registers a callback for the given path for PUT requests.
    pub fn put(&mut self, path: &str, callback: Instance<T>) {
        self.insert(Put, path, callback);
    }

    /// Registers a callback for the given path for DELETE requests.
    pub fn delete(&mut self, path: &str, callback: Instance<T>) {
        self.insert(Delete, path, callback);
    }

    /// Registers a callback for the given path for HEAD requests.
    pub fn head(&mut self, path: &str, callback: Instance<T>) {
        self.insert(Head, path, callback);
    }

    /// Registers a static callback for the given path for GET requests.
    pub fn get_static(&mut self, path: &str, callback: Static) {
        self.insert(Get, path, callback);
    }

    /// Inserts the given callback for the given method and given route.
    pub fn insert<I: Into<Callback<T>>>(&mut self, method: Method, path: &str, callback: I) {
        self.router.insert(method, path, callback.into())
    }

    // Registers a template with the given name.
    pub fn register_template(&mut self, name: &str) {
        let mut path = PathBuf::new();
        path.push("views");
        path.push(name);
        path.set_extension("hbs");

        self.handlebars.register_template_file(name, &path).unwrap();
    }

}

/// Defines an impl that creates a new instance of `T` for each request using
/// `Default::default`.
impl<T: Default + Send> Edge<T> {

    /// Runs the server in one thread per cpu.
    ///
    /// Creates one instance of `T` per request by calling `Default::default`.
    /// This method blocks the current thread.
    pub fn start(&mut self) -> IoResult<()> {
        // get address and start listening
        let addr = self.router.base_url.to_socket_addrs().unwrap().next().unwrap();
        let listener = HttpListener::bind(&addr).unwrap();

        // 50% threads for the pool, 50% for the listeners
        let num_threads = ::std::cmp::max(num_cpus::get() / 2, 1);
        let pool = Pool::new(num_threads);
        pool.scoped(|pool_scope| {
            crossbeam::scope(|scope| {
                for i in 0..num_threads {
                    let listener = listener.try_clone().unwrap();
                    let router = &self.router;
                    let handlebars = &self.handlebars;
                    scope.spawn(move || {
                        info!("thread {} listening on http://{}", i, addr);
                        Server::new(listener).handle(move |control| {
                            let app = T::default();
                            handler::EdgeHandler::new(pool_scope, app, &router, &handlebars, control)
                        }).unwrap();
                    });
                }
            });
        });

        Ok(())
    }
}

/// Defines an impl that creates a new instance of `T` for each request
/// by cloning an initial instance of `T`.
impl<T: Clone + Send + Sync> Edge<T> {

    /// Runs the server in one thread per cpu.
    ///
    /// Creates one instance of `T` per request by cloning `app`.
    /// This method blocks the current thread.
    pub fn start_with(&mut self, app: T) -> IoResult<()> {
        // get address and start listening
        let addr = self.router.base_url.to_socket_addrs().unwrap().next().unwrap();
        let listener = HttpListener::bind(&addr).unwrap();

        // 50% threads for the pool, 50% for the listeners
        let num_threads = ::std::cmp::max(num_cpus::get() / 2, 1);
        let pool = Pool::new(num_threads);
        pool.scoped(|pool_scope| {
            crossbeam::scope(|scope| {
                for i in 0..num_threads {
                    let listener = listener.try_clone().unwrap();
                    let router = &self.router;
                    let handlebars = &self.handlebars;
                    let app = &app;
                    scope.spawn(move || {
                        info!("thread {} listening on http://{}", i, addr);
                        Server::new(listener).handle(move |control| {
                            handler::EdgeHandler::new(pool_scope, app.clone(), &router, &handlebars, control)
                        }).unwrap();
                    });
                }
            });
        });

        Ok(())
    }
}

fn render_html(text: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(OPTION_ENABLE_TABLES);
    opts.insert(OPTION_ENABLE_FOOTNOTES);

    let mut s = String::with_capacity(text.len() * 3 / 2);
    let p = Parser::new_ext(text, opts);
    html::push_html(&mut s, p);
    s
}

/// this code is based on code Copyright (c) 2015 Wayne Nilsen
/// see https://github.com/waynenilsen/handlebars-markdown-helper/blob/master/src/lib.rs#L31
///
/// because the handlebars-markdown-helper crate does not allow custom options for Markdown rendering yet
fn markdown_helper(_: &Context, h: &Helper, _ : &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
    let markdown_text_var = try!(h.param(0).ok_or_else(|| RenderError::new(
        "Param not found for helper \"markdown\"")
    ));
    let markdown = try!(markdown_text_var.value().as_string().ok_or_else(||
        RenderError::new(format!("Expected a string for parameter {:?}", markdown_text_var))
    ));
    let html = render_html(markdown);
    try!(rc.writer.write_all(html.as_bytes()));
    Ok(())
}

fn init_handlebars(handlebars: &mut Handlebars) -> IoResult<()> {
    // register markdown helper
    handlebars.register_helper("markdown", Box::new(::markdown_helper));

    // register partials folder (if it exists)
    let partials = Path::new("views/partials");
    if partials.exists() {
        for it in try!(read_dir("views/partials")) {
            let entry = try!(it);
            let path = entry.path();
            if path.extension().is_some() && path.extension().unwrap() == "hbs" {
                let name = path.file_stem().unwrap().to_str().unwrap();
                handlebars.register_template_file(name, path.as_path()).unwrap();
            }
        }
    }

    Ok(())
}
