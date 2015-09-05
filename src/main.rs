extern crate rustc_serialize;
use std::io::prelude::*;
use std::error::Error;
use rustc_serialize::json::{self, Json, ToJson};
use rustc_serialize::Encodable;

extern crate iron;
use iron::Iron;
use iron::method::Method;
use iron::status::Status;
use iron::response::Response;
use iron::request::Request;

extern crate router;
use router::Router;

extern crate rethinkdb;
use rethinkdb::r::*;
use rethinkdb::RethinkDB;

extern crate rand;


mod db {
    extern crate plugin;
    extern crate typemap;
    extern crate rethinkdb;

    use std::sync::{Mutex, Arc, MutexGuard};
    use self::plugin::Extensible;

    use rethinkdb::RethinkDB;



    /*
    pub struct RethinkdbMiddleware {
        pub pool: Arc<Mutex<rethinkdb::RethinkDB>>,
    }

    impl RethinkdbMiddleware {
        pub fn new(host: &str, port: u16, auth: &str, connections: usize) ->
            RethinkdbMiddleware {
            let db = RethinkDB::connect(host, port, auth, connections);

            RethinkdbMiddleware {
                pool: Arc::new(Mutex::new(db)),
            }
        }
    }

    struct Key;
    impl Middleware for RethinkdbMiddleware {
        fn invoke<'a>(&self, req: &mut Request, res: Response<'a>) ->
            MiddlewareResult<'a> {
                req.extensions_mut().insert::<Key>(self.pool.clone());
                Ok(Continue(res))
        }
    }

    impl typemap::Key for Key {
        type Value = Arc<Mutex<RethinkDB>>;
    }

    pub trait RethinkdbRequestExtensions<'a> {
        fn db_conn(&self) -> MutexGuard<RethinkDB>;
    }

    impl<'a, 'b, 'c> RethinkdbRequestExtensions<'a> for Request<'a, 'b, 'c> {
        fn db_conn(&self) -> MutexGuard<RethinkDB> {
            self.extensions().get::<Key>().unwrap().lock().unwrap()
        }

    }
    */
}
//use db::RethinkdbRequestExtensions;

mod error {
    use std::convert::From; 
    use std::fmt::{self, Display, Formatter};

    extern crate rethinkdb;
    use rethinkdb::RethinkDBError;

    extern crate rustc_serialize;
    use rustc_serialize::json::EncoderError;

    use std::error::Error;

    use std::io;
    use std::rc::Rc;

    #[derive(Debug)]
    pub struct E {
        description: String,
    }

    impl Display for E {
        fn fmt(&self, f : &mut Formatter) -> Result<(), fmt::Error> {
            self.description().fmt(f)
        }

    }

    impl Error for E {
        fn description(&self) -> &str {
            &self.description[..]
        }
    }

    impl From<RethinkDBError> for E {
        fn from(err : RethinkDBError) -> E {
            E{description: format!("{:?}", err)}
        }
    }

    impl From<io::Error> for E {
        fn from(err : io::Error) -> E {
            E{description: format!("{:?}", err)}
        }
    }

    impl From<Rc<io::Error>> for E {
        fn from(err : Rc<io::Error>) -> E {
            E{description: format!("{:?}", err)}
        }
    }

    impl From<EncoderError> for E {
        fn from(err : EncoderError) ->  E {
            E{description: format!("{:?}", err)}
        }
    }
}

mod models {
    pub mod auth {
        extern crate rand;
        use rand::Rng;

        use rustc_serialize::json::{self, ToJson};

        #[derive(RustcEncodable)]
        pub struct Token {
            token: String
        }

        impl Token {
            pub fn new() -> Token {
                let mut rnd = rand::OsRng::new().unwrap();
                let token = rnd.gen_ascii_chars().take(64).collect();
                Token{token: token}
            }

            pub fn to_json(&self) -> json::Json {
                json::Json::String(self.token.clone())
            }
        }
    }

    pub mod user {
        extern crate rethinkdb;
        use rethinkdb::RethinkDB;
        use rethinkdb::r::*;

        use std::rc::Rc;
        use super::auth;

        use super::super::error::E;

        #[derive(Debug, RustcDecodable, RustcEncodable)]
        pub struct User {
            name: String,
        }

        use std::collections::BTreeMap;
        use rustc_serialize::json::{self, ToJson};
        impl  User {
            pub fn to_map(&self) -> BTreeMap<String, json::Json> {
                let mut d = BTreeMap::new();
                d.insert("name".to_string(), self.name.to_json());
                d
            }

            pub fn insert(&self, mut conn: &mut RethinkDB) -> Result<auth::Token, E> {
                let mut map = self.to_map();
                let token = auth::Token::new();
                map.insert("auth_token".to_string(), token.to_json());
                try!(db("test").table("users").
                     insert(vec![json::Json::Object(map)]).run(&mut conn));
                Ok(token)
            }
        }
    }
}

/*
fn add_user(request: &mut nickel::Request) -> Result<models::auth::Token, error::E> {
    let user = try!(request.json_as::<models::user::User>());
    let mut conn = request.db_conn();
    let token = try!(user.insert(&mut *conn));
    Ok(token)
}

fn response<T: Encodable>(r: Result<T,error::E>) -> (StatusCode, String) {
    match r.and_then(|t| json::encode(&t).map_err(|e| error::E::from(e))) {
        Ok(t) => (StatusCode::Ok, t),
        Err(e) => (StatusCode::BadRequest, e.description().to_string()),
    }
}
*/

fn main() {
    //let dbmiddleware = db::RethinkdbMiddleware::new("localhost", 28015, "", 5);
    //server.utilize(dbmiddleware);


    let mut router = Router::new();


    router.route(Method::Get, "/user/:userid", |req: &mut Request| {
        let ref query = req.extensions.get::<Router>().unwrap().find("userid").unwrap();
        Ok(Response::with((Status::Ok, format!("This is user: {}", query))))
    });


    router.route(Method::Post, "/users", |request: &mut Request| {
        response(add_user(request))
    });

    Iron::new(router).http("localhost:6767").unwrap();
}
