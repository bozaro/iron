//! Iron's HTTP Request representation and associated methods.

use std::io::{self, Read};
use std::net::SocketAddr;
use std::fmt::{self, Debug};

use hyper::uri::RequestUri::{AbsoluteUri, AbsolutePath};
use hyper::net::NetworkStream;
use hyper::http::h1::HttpReader;

use typemap::TypeMap;
use plugin::Extensible;
use method::Method;

pub use hyper::server::request::Request as HttpRequest;
use hyper::buffer;

pub use self::url::Url;

use {Protocol, Plugin, Headers, Set, headers};

mod url;

/// The `Request` given to all `Middleware`.
///
/// Stores all the properties of the client's request plus
/// an `TypeMap` for data communication between middleware.
pub struct Request<'a, 'b: 'a> {
    /// The requested URL.
    pub url: Url,

    /// The originating address of the request.
    pub remote_addr: SocketAddr,

    /// The local address of the request.
    pub local_addr: SocketAddr,

    /// The request headers.
    pub headers: Headers,

    /// The request body as a reader.
    pub body: Body<'a, 'b>,

    /// The request method.
    pub method: Method,

    /// Extensible storage for data passed between middleware.
    pub extensions: TypeMap
}

impl<'a, 'b> Debug for Request<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(f, "Request {{"));

        try!(writeln!(f, "    url: {:?}", self.url));
        try!(writeln!(f, "    method: {:?}", self.method));
        try!(writeln!(f, "    remote_addr: {:?}", self.remote_addr));
        try!(writeln!(f, "    local_addr: {:?}", self.local_addr));

        try!(write!(f, "}}"));
        Ok(())
    }
}

impl<'a, 'b> Request<'a, 'b> {
    /// Create a request from an HttpRequest.
    ///
    /// This constructor consumes the HttpRequest.
    pub fn from_http(req: HttpRequest<'a, 'b>, local_addr: SocketAddr, protocol: &Protocol)
                     -> Result<Request<'a, 'b>, String> {
        let url = try! (Request::prepare_uri(&req, local_addr, protocol));
        let (addr, method, headers, _, _, reader) = req.deconstruct();

        Ok(Request {
            url: url,
            remote_addr: addr,
            local_addr: local_addr,
            headers: headers,
            body: Body::new(reader),
            method: method,
            extensions: TypeMap::new()
        })
    }

    /// Create a request from an incompleted HttpRequest.
    pub fn from_header(req: &HttpRequest, local_addr: SocketAddr, protocol: &Protocol) -> Result<Request<'static, 'static>, String> {
        let url = try! (Request::prepare_uri(&req, local_addr, protocol));
        Ok(Request {
            url: url,
            remote_addr: req.remote_addr,
            local_addr: local_addr,
            headers: req.headers.clone(),
            body: Body::empty(),
            method: req.method.clone(),
            extensions: TypeMap::new()
        })
    }

    fn prepare_uri(req: &HttpRequest, local_addr: SocketAddr, protocol: &Protocol) -> Result<Url, String> {
        match &req.uri {
            &AbsoluteUri(ref url) => {
                Url::from_generic_url(url.clone())
            },
            &AbsolutePath(ref path) => {
                // Attempt to prepend the Host header (mandatory in HTTP/1.1)
                match req.headers.get::<headers::Host>() {
                    Some(ref host) => {
                        let url_string = format!("{}://{}:{}{}", protocol.name(), host.hostname, local_addr.port(),
                                path);
                        Url::parse(&url_string)
                    },
                    None => Err("No host specified in request".into())
                }
            },
            _ => Err("Unsupported request URI".into())
        }
    }
}

/// The body of an Iron request,
pub struct Body<'a, 'b: 'a>(Option<HttpReader<&'a mut buffer::BufReader<&'b mut NetworkStream>>>);

impl<'a, 'b> Body<'a, 'b> {
    /// Create a new reader for use in an Iron request from a hyper HttpReader.
    pub fn new(reader: HttpReader<&'a mut buffer::BufReader<&'b mut NetworkStream>>) -> Body<'a, 'b> {
        Body(Some(reader))
    }

    /// Create a new fake reader.
    pub fn empty() -> Body<'a, 'b> {
        Body(None)
    }
}

impl<'a, 'b> Read for Body<'a, 'b> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.as_mut().ok_or(io::Error::new(io::ErrorKind::InvalidInput, "This request don't have body yet"))
            .and_then(|ref mut r| r.read(buf))
    }
}

// Allow plugins to attach to requests.
impl<'a, 'b> Extensible for Request<'a, 'b> {
    fn extensions(&self) -> &TypeMap {
        &self.extensions
    }

    fn extensions_mut(&mut self) -> &mut TypeMap {
        &mut self.extensions
    }
}

impl<'a, 'b> Plugin for Request<'a, 'b> {}
impl<'a, 'b> Set for Request<'a, 'b> {}
