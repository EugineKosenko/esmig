/*
 * ESMig - Elasticsearch Migration Tool
 * Copyright (C) 2025 Eugine Kosenko
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program; if not, write to the Free Software Foundation, Inc.,
 * 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
 */

#[macro_use] extern crate clap;
#[macro_use] extern crate serde_json;

use elasticsearch::{
    Elasticsearch,
    auth::Credentials,
    http::transport::{TransportBuilder, SingleNodeConnectionPool}
};

use clap::Parser as ClapParser;
use std::fs;
use elasticsearch::indices::{IndicesExistsParts, IndicesCreateParts};
use std::io::Write;
use nom::{
    Parser as NomParser,
    character::complete::multispace0,
    error::ParseError,
    sequence::delimited
};
use nom::{
    IResult,
    branch::alt,
    character::complete::{char, not_line_ending, line_ending, multispace1},
    combinator::map,
    multi::many0
};
use nom::{
    bytes::complete::tag,
    combinator::value
};
use nom::{
    bytes::complete::escaped_transform,
    character::complete::none_of
};
use nom::combinator::opt;
use nom::{
    bytes::complete::is_not,
    character::complete::i128,
    combinator::peek,
    number::complete::double
};
use nom::multi::separated_list0;
use nom::sequence::separated_pair;
use nom::character::complete::space1;
use nom::{
    sequence::terminated,
    combinator::eof
};
use std::env;
use elasticsearch::{CountParts, IndexParts};
use elasticsearch::{SearchParts, DeleteByQueryParts};

#[derive(ClapParser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Create { mgrtn: String },
    Run, Revert, Redo
}
fn ws<'a, F, O, E>(inner: F) -> impl NomParser<&'a str, Output = O, Error = E>
where
    F: NomParser<&'a str, Output = O, Error = E>,
    E: ParseError<&'a str>,
{
    delimited(multispace0, inner, multispace0)
}
fn skip(input: &str) -> IResult<&str, ()> {
    map(many0(
        alt((
            delimited(char('#'), not_line_ending, line_ending),
            multispace1
        )))
        , |_| ()).parse(input)
}
#[derive(Clone, Debug)]
enum Method { Put, Delete }
fn method(input: &str) -> IResult<&str, Method> {
    alt((
        value(Method::Put, tag("PUT")),
        value(Method::Delete, tag("DELETE"))
    )).parse(input)
}
fn content(input: &str) -> IResult<&str, String> {
    escaped_transform(
        none_of("\"\\\n\r\t"), '\\',
        alt((
            value("\"", char('"')),
            value("\\", char('\\')),
            value("\n", char('n')),
            value("\r", char('r')),
            value("\t", char('t'))
        ))
    ).parse(input)
}
fn str0(input: &str) -> IResult<&str, String> {
    ws(delimited(
        char('"'),
        map(opt(content), |s| s.unwrap_or_else(String::new)),
        char('"'))
    ).parse(input)
}

fn str1(input: &str) -> IResult<&str, String> {
    ws(delimited(char('"'), content, char('"'))).parse(input)
}
fn jval(input: &str) -> IResult<&str, serde_json::Value> {
    alt((jnull, jbool, jnum, jstr, jarr, jobj)).parse(input)
}
fn jnull(input: &str) -> IResult<&str, serde_json::Value> {
    map(tag("null"), |_| serde_json::Value::Null).parse(input)
}
fn jbool(input: &str) -> IResult<&str, serde_json::Value> {
    map(alt((
        value(false, tag("false")),
        value(true, tag("true")))),
        |b| serde_json::Value::Bool(b))
        .parse(input)
}
fn jnum(input: &str) -> IResult<&str, serde_json::Value> {
    let (_, ahead) = peek(is_not(" \t\n\r")).parse(input)?;
    if ahead.contains('.') || ahead.to_lowercase().contains('e') {
        map(double, |f| serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap())).parse(input)
    } else {
        map(i128, |i| serde_json::Value::Number(serde_json::Number::from_i128(i).unwrap())).parse(input)
    }
}
fn jstr(input: &str) -> IResult<&str, serde_json::Value> {
    map(str0, |s| serde_json::Value::String(s)).parse(input)
}
fn jarr(input: &str) -> IResult<&str, serde_json::Value> {
    map(delimited(
        ws(char('[')),
        separated_list0(ws(char(',')), jval),
        ws(char(']'))),
        |a| serde_json::Value::Array(a))
        .parse(input)
}
fn jobj(input: &str) -> IResult<&str, serde_json::Value> {
    let field = separated_pair(str1, ws(char(':')), jval);
    map(delimited(
        ws(char('{')),
        separated_list0(ws(char(',')), field),
        ws(char('}'))),
        |fields| serde_json::Value::Object(fields.into_iter().collect()))
        .parse(input)
}
#[derive(Debug)]
struct Command(Method, String, serde_json::Value);
fn command(input: &str) -> IResult<&str, Command> {
    map((method, space1, not_line_ending, line_ending, opt(jobj)),
        |(method, _, route, _, body)| Command(method, route.to_string(), body.unwrap_or(serde_json::Value::Null))
    ).parse(input)
}
#[derive(Debug)]
struct Script(Vec<Command>);
fn script(input: &str) -> IResult<&str, Script> {
    map(terminated(many0(delimited(skip, command, skip)), eof), |cmds| Script(cmds)).parse(input)
}
async fn process_mgrtn(mgrtn: &str, dir: &str) {
    let http = reqwest::Client::new();
    let content = fs::read_to_string(format!("es-migrations/{}/{}.esm", mgrtn, dir)).unwrap();
    let script = script(&content).unwrap().1;
    for Command(method, route, body) in script.0 {
        let route = route.replace("{circuit}", &env::var("CIRCUIT").unwrap());
        let url = format!("{}{}", env::var("ES_ROOT").unwrap(), route);
        let request = match method {
            Method::Put => http.put(url).json(&body),
            Method::Delete => http.delete(url)
        };
        let response = request
            .basic_auth(env::var("ES_USER").unwrap(), Some(env::var("ES_PASS").unwrap()))
            .send().await.unwrap();
        println!("{:?}", response.json::<serde_json::Value>().await.unwrap());
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let es = Elasticsearch::new(
        TransportBuilder::new(
            SingleNodeConnectionPool::new(
                reqwest::Url::parse(&env::var("ES_ROOT").unwrap()).unwrap()))
            .auth(Credentials::Basic(env::var("ES_USER").unwrap(), env::var("ES_PASS").unwrap()))
            .build().unwrap());
    let mgrtns_index_name = format!(".{}-esmigs", env::var("CIRCUIT").unwrap());
    let args = Args::parse();
    
    match args.command {
        Commands::Init => {
            let response = es.indices()
                .exists(IndicesExistsParts::Index(&[&mgrtns_index_name]))
                .send().await.unwrap();
            
            match response.status_code() {
                reqwest::StatusCode::OK => { println!("Index created"); },
                reqwest::StatusCode::NOT_FOUND => {
                    println!("Create index {}", mgrtns_index_name);
                    let response = es.indices()
                        .create(IndicesCreateParts::Index(&mgrtns_index_name))
                        .body(json!({
                            "settings": {
                                "index.hidden": true
                            },
                            "mappings": {
                                "properties": {
                                    "stamp": { "type": "date" },
                                    "version": { "type": "keyword" }
                                }
                            }
                        }))
                        .send().await.unwrap();
                    println!("{:?}", response.status_code());
                },
                code => {
                    println!("{:?}", code);
                    println!("{:?}", response.text().await.unwrap());
                }
            }
        },
        Commands::Create { mgrtn } => {
            let version = chrono::Local::now().format("%Y-%m-%d-%H%M%S-0000").to_string();
            let dir = format!("es-migrations/{}_{}", version, mgrtn);
            fs::create_dir(&dir).unwrap();
            let mut file = fs::File::create(format!("{}/up.esm", dir)).unwrap();
            file.write_all("# Your ESM goes here".as_bytes()).unwrap();
            let mut file = fs::File::create(format!("{}/down.esm", dir)).unwrap();
            file.write_all("# This file should undo anything in `up.esm`".as_bytes()).unwrap();
            println!("{}", dir);
        },
        Commands::Run => {
            let mut mgrtns: Vec<_> = fs::read_dir("es-migrations").unwrap()
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.path().file_name().map(|name| name.to_string_lossy().to_string()))
                .collect();
            mgrtns.sort();
            for mgrtn in mgrtns {
                let version = mgrtn.split('_').next().unwrap();
                let count = es.count(CountParts::Index(&[&mgrtns_index_name]))
                    .body(json!({ "query": { "term": { "version": { "value": version }}}}))
                    .send().await.unwrap()
                    .json::<serde_json::Value>().await.unwrap()["count"].as_u64().unwrap();
                if count == 0 {
                    process_mgrtn(&mgrtn, "up").await;
                
                    let stamp = chrono::Utc::now().naive_utc();
                    es.index(IndexParts::Index(&mgrtns_index_name))
                      .body(json!({ "stamp": stamp, "version": version }))
                      .send().await.unwrap();
                }
            }
        },
        Commands::Revert => {
            let result = es.search(SearchParts::Index(&[&mgrtns_index_name]))
                .body(json!({ "size": 1, "sort": [{ "stamp": { "order": "desc" } }] }))
                .send().await.unwrap()
                .json::<serde_json::Value>().await.unwrap();
            
            if let Some(version) = result.pointer("/hits/hits/0/_source/version").map(|v| v.as_str().unwrap()) {
                let mgrtn = glob::glob(&format!("es-migrations/{}_*", version)).unwrap()
                    .filter_map(|entry| entry.ok())
                    .filter(|path| path.is_dir())
                    .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
                    .next().unwrap();
                
                process_mgrtn(&mgrtn, "down").await;
            
                es.delete_by_query(DeleteByQueryParts::Index(&[&mgrtns_index_name]))
                    .body(json!({ "query": { "match": { "version": version } } }))
                    .send().await.unwrap();
            }
        },
        Commands::Redo => {
            let result = es.search(SearchParts::Index(&[&mgrtns_index_name]))
                .body(json!({ "size": 1, "sort": [{ "stamp": { "order": "desc" } }] }))
                .send().await.unwrap()
                .json::<serde_json::Value>().await.unwrap();
            
            if let Some(version) = result.pointer("/hits/hits/0/_source/version").map(|v| v.as_str().unwrap()) {
                let mgrtn = glob::glob(&format!("es-migrations/{}_*", version)).unwrap()
                    .filter_map(|entry| entry.ok())
                    .filter(|path| path.is_dir())
                    .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
                    .next().unwrap();
                
                process_mgrtn(&mgrtn, "down").await;
            
                es.delete_by_query(DeleteByQueryParts::Index(&[&mgrtns_index_name]))
                    .body(json!({ "query": { "match": { "version": version } } }))
                    .send().await.unwrap();
            }
            let mut mgrtns: Vec<_> = fs::read_dir("es-migrations").unwrap()
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.path().file_name().map(|name| name.to_string_lossy().to_string()))
                .collect();
            mgrtns.sort();
            for mgrtn in mgrtns {
                let version = mgrtn.split('_').next().unwrap();
                let count = es.count(CountParts::Index(&[&mgrtns_index_name]))
                    .body(json!({ "query": { "term": { "version": { "value": version }}}}))
                    .send().await.unwrap()
                    .json::<serde_json::Value>().await.unwrap()["count"].as_u64().unwrap();
                if count == 0 {
                    process_mgrtn(&mgrtn, "up").await;
                
                    let stamp = chrono::Utc::now().naive_utc();
                    es.index(IndexParts::Index(&mgrtns_index_name))
                      .body(json!({ "stamp": stamp, "version": version }))
                      .send().await.unwrap();
                }
            }
        }
    }
}
