#[macro_use] extern crate juniper;

#[macro_use] extern crate diesel_codegen;
#[macro_use] extern crate diesel;

extern crate iron;
extern crate mount;
extern crate logger;
extern crate persistent;

extern crate dotenv;

extern crate r2d2;
extern crate r2d2_diesel;

use std::env;
use dotenv::dotenv;

use iron::prelude::*;
use iron::typemap::Key;
use persistent::Read;
use juniper::iron_handlers::{GraphQLHandler, GraphiQLHandler};

use r2d2_diesel::ConnectionManager;
use diesel::pg::PgConnection;

mod db;
mod models;
mod schema;

pub type PostgresPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub struct DbConn;
impl Key for DbConn { type Value = PostgresPool; }

/*
A context object is used in Juniper to provide out-of-band access to global
data when resolving fields. We use it here to pass a database connection
to the Query and Mutation types.

Since this function is called once for every request, it will create a
database connection per request. A more realistic solution would be to use
the "r2d2" crate for connection pooling, and the "persistent" crate to pass
data into Iron requests.
*/
fn context_factory(req: &mut Request) -> schema::Context {
    // schema::Context {
    //     connection: db::establish_connection(),
    // }
    let pool = req.get::<Read<DbConn>>().unwrap(); // the pool gets passed alright
    let conn = pool.get();
    
    schema::Context {
        connection: *conn.unwrap(), // but trying to deref r2d2 pooledconnection to pgconnection results in "cannot move out of borrowed content"
    }
}

fn main() {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let config = r2d2::Config::default();
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::new(config, manager).expect("Failed to create pool.");

    let graphql_endpoint = GraphQLHandler::new(
        context_factory,
        schema::QueryRoot,
        schema::MutationRoot,
    );

    let graphiql_endpoint = GraphiQLHandler::new("/graphql");

    let mut mount = mount::Mount::new();
    mount.mount("/", graphiql_endpoint);
    mount.mount("/graphql", graphql_endpoint);

    let (logger_before, logger_after) = logger::Logger::new(None);

    let mut chain = Chain::new(mount);
    chain.link(Read::<DbConn>::both(pool));
    chain.link_before(logger_before);
    chain.link_after(logger_after);

    let host = env::var("LISTEN").unwrap_or("0.0.0.0:8080".to_owned());
    println!("GraphQL server started on {}", host);
    Iron::new(chain).http(host.as_str()).unwrap();
}
