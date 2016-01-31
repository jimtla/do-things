#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

use std::error::Error;

#[macro_use]
extern crate iron;
use iron::Iron;
use iron::method::Method;
use iron::status::Status;
use iron::response::Response;
use iron::request::Request;

extern crate router;
use router::Router;

extern crate rethink;
use rethink::Rethink;

extern crate rand;


extern crate serde_json;
extern crate serde;

mod error {
    use std::convert::From;
    use std::fmt::{self, Display, Formatter};

    use std::error::Error;

    use std::io;
    use std::rc::Rc;

    use rethink;

    #[derive(Debug)]
    pub struct E {
        description: String,
    }

    impl E {
        pub fn new(description: String) -> Self {
            E { description: description }
        }
    }

    impl Display for E {
        fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
            self.description().fmt(f)
        }
    }

    impl Error for E {
        fn description(&self) -> &str {
            &self.description[..]
        }
    }

    impl From<io::Error> for E {
        fn from(err: io::Error) -> E {
            E { description: format!("{:?}", err) }
        }
    }

    impl From<Rc<io::Error>> for E {
        fn from(err: Rc<io::Error>) -> E {
            E { description: format!("{:?}", err) }
        }
    }

    impl From<rethink::Error> for E {
        fn from(err: rethink::Error) -> E {
            E { description: format!("{:?}", err) }
        }
    }
}

mod models {

    pub mod persisted {
        use rethink::{Datum, Connection, Rethink, Response_ResponseType};
        use super::super::error::E;

        #[derive(Debug, PartialEq, Eq, Clone)]
        pub struct Id(String);
        impl Id {
            fn str(&self) -> &str {
                self.0.as_str()
            }

            pub fn from_string<S: Into<String>>(id: S) -> Self {
                Id(id.into())
            }
        }

        pub trait Persistable : Sized {
            fn table() -> &'static str;
            fn to_db(&self) -> Datum;
            fn from_db(&Datum) -> Result<Self, E>;

            fn insert(self, c: &mut Connection) -> Result<Persisted<Self>, E> {
                let d = self.to_db();
                let res = try!(Rethink::table(Self::table()).insert(&d, None).run(c));
                if res.response_type == Response_ResponseType::RUNTIME_ERROR {
                    return Err(E::new(format!("{:?}", res.result)));
                }

                let id = if let Some(&Datum::Object(ref r)) = res.result.get(0) {
                    if let Some(&Datum::Array(ref keys)) = r.get("generated_keys") {
                        if let Some(&Datum::String(ref key)) = keys.get(0) {
                            Some(key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(key) = id {
                    Ok(Persisted {
                        id: Id(key.clone()),
                        contents: self,
                    })
                } else {
                    Err(E::new(format!("Unexpected Result: {:?}", res.result)))
                }
            }

            fn get(c: &mut Connection, id: &Id) -> Result<Option<Persisted<Self>>, E> {
                let res = try!(Rethink::table(Self::table()).get(id.str()).run(c));
                if res.response_type == Response_ResponseType::RUNTIME_ERROR {
                    return Err(E::new(format!("{:?}", res.result)));
                }
                println!("{:?}", res);

                let r = res.result.get(0).map(|o| {
                    Self::from_db(o).map(|c| {
                        Persisted {
                            id: id.clone(),
                            contents: c,
                        }
                    })
                });

                match r {
                    None => Ok(None),
                    Some(Ok(p)) => Ok(Some(p)),
                    Some(Err(e)) => Err(e),
                }
            }
        }

        #[derive(Debug)]
        pub struct Persisted<T: Persistable> {
            pub id: Id,
            pub contents: T,
        }
    }

    pub mod auth {
        extern crate rand;
        use rand::Rng;

        pub struct Token {
            token: String,
        }

        impl Token {
            pub fn new() -> Token {
                let mut rnd = rand::OsRng::new().unwrap();
                let token = rnd.gen_ascii_chars().take(64).collect();
                Token { token: token }
            }
        }
    }

    pub mod user {
        use super::super::error::E;

        use rethink::Datum;
        use super::persisted::Persistable;


        #[derive(Debug, Serialize, Deserialize)]
        pub struct User {
            name: String,
        }

        impl User {
            pub fn new<S: Into<String>>(name: S) -> Self {
                User { name: name.into() }
            }
        }

        impl Persistable for User {
            fn table() -> &'static str {
                "users"
            }

            fn to_db(&self) -> Datum {
                Datum::Object(vec![("name".into(), Datum::String(self.name.clone()))]
                                  .into_iter()
                                  .collect())
            }

            fn from_db(d: &Datum) -> Result<Self, E> {
                let user = if let Datum::Object(ref o) = *d {
                    o.get("name").and_then(|name| {
                        if let Datum::String(ref s) = *name {
                            Some(User { name: s.clone() })
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };

                user.ok_or_else(|| E::new(format!("Couldn't make user from {:?}", d)))
            }
        }

        #[cfg(test)]
        mod tests {
            extern crate rand;
            use rand::Rng;

            use rethink::Rethink;

            use super::User;
            use super::super::persisted::Persistable;

            #[test]
            fn test_insert() {
                let mut rnd = rand::OsRng::new().unwrap();
                let name: String = rnd.gen_ascii_chars().take(64).collect();

                let mut rc = Rethink::connect_default().unwrap();
                let user = User::new(name);
                let persisted_user = user.insert(&mut rc).expect("insert the user");
                println!("{:?}", persisted_user);

                let fetched = User::get(&mut rc, &persisted_user.id)
                                  .expect("get the user")
                                  .unwrap();
                assert_eq!(fetched.id, persisted_user.id);
                assert_eq!(fetched.contents.name, persisted_user.contents.name);
            }
        }
    }

    pub mod event {
        use super::user::User;

        #[derive(Debug, Serialize, Deserialize)]
        pub enum Guest {
            Invited(User),
            Attending(User),
            Rejected(User),
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Event {
            name: String,
            guests: Vec<Guest>,
            id: String,
        }
    }
}

fn main() {
    use models::persisted::{Persistable, Id};

    let mut router = Router::new();

    router.route(Method::Get, "/user/:userid", |req: &mut Request| {
        let mut rc = Rethink::connect_default().unwrap();
        let query = req.extensions.get::<Router>().unwrap().find("userid").unwrap();
        let user = itry!(models::user::User::get(&mut rc, &Id::from_string(query)));
        Ok(Response::with((Status::Ok, format!("This is user: {:?}", user))))
    });


    router.route(Method::Post, "/users", |request: &mut Request| {
        #[derive(Debug, Deserialize)]
        struct Body {
            name: String,
        }

        let mut rc = Rethink::connect_default().unwrap();

        let body: Body = itry!(serde_json::de::from_reader(&mut request.body));
        let user = models::user::User::new(body.name);
        let persisted_user = itry!(user.insert(&mut rc));
        Ok(Response::with((Status::Ok, format!("Creating a user: {:?}", persisted_user))))
    });

    println!("Running...");
    Iron::new(router).http("localhost:6767").unwrap();
}
