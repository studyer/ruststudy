use std::{collections::HashMap, str::FromStr};
use clap::Parser;
use colored::Colorize;
use reqwest::{header, Client, Response, Url};
use anyhow::{anyhow, Result};
use mime::Mime;
use syntect::{
    easy::HighlightLines,
    highlighting::{ThemeSet, Style},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};


// 定义 HTTPie 的 CLI 的主入口，它包含若干个子命令
// 下面 /// 的注释是文档，clap 会将其作为 CLI 的帮助

/// A naive httpie implementation with Rust, can you imagine how easy it is?
#[derive(Parser, Debug)]
#[clap(version = "1.0", author = "Tyr Chen <tyr@chen.com>")]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

// 子命令分别对应不同的 HTTP 方法，目前只支持 get / post
#[derive(Parser, Debug)]
enum SubCommand {
    Get(Get),
    Post(Post),
    // 我们暂且不支持其它 HTTP 方法
}

// get 子命令

/// feed get with an url and we will retrieve the response for you
#[derive(Parser, Debug)]
struct Get {
    /// HTTP 请求的 URL
    #[clap(value_parser = parse_url)]
    url: String,
}

// post 子命令。需要输入一个 URL，和若干个可选的 key=value，用于提供 json body

/// feed post with an url and optional key=value pairs. We will post the data
/// as JSON, and retrieve the response for you
#[derive(Parser, Debug)]
struct Post {
    /// HTTP 请求的 URL
    #[clap(value_parser = parse_url)]
    url: String,
    /// HTTP 请求的 body
    #[clap(value_parser = parse_kv_pair)]
    body: Vec<KvPair>,
}

#[derive(Debug, PartialEq, Clone)]
struct KvPair {
    k: String,
    v: String,
}

impl FromStr for KvPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 使用 = 进行split，这会得到一个迭代器
        let mut split = s.split('=');
        let err = || anyhow!("Failed to parse {}", s);
        Ok(Self {
            // 从迭代器中取第一个结果作为key，迭代器返回Some(T)/None
            // 我们将其转换成Ok(T)/Err(E)，然后用?处理错误
            k: (split.next().ok_or_else(err)?).to_string(),
            // 从迭代器中取第二个结果作为value
            v: (split.next().ok_or_else(err)?).to_string(),
        })
    }
}

/// 因为我们为 KvPair 实现了 FromStr，这里可以直接 s.parse() 得到 KvPair
fn parse_kv_pair(s: &str) -> Result<KvPair> {
    Ok(s.parse()?)
}

fn parse_url(s: &str) -> Result<String> {
    // 这里我们仅仅检查一下url是否合法
    let _url: Url = s.parse()?;
    Ok(s.into())
}

fn print_syntect(body: &str, ext: &str) {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ps.find_syntax_by_extension(ext).unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    for line in LinesWithEndings::from(body) {
        let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        print!("{}", escaped);
    }
}

fn print_status(resp: &Response) {
    if resp.status().is_client_error() {
        println!("{}", resp.status().to_string().red());
    } else if resp.status().is_server_error() {
        println!("{}", resp.status().to_string().red());
    } else {
        let status = format!("{:?} {}", resp.version(), resp.status()).green();
        println!("{}\n", status);
    }
}

fn print_headers(resp: &Response) {
    for (name, value) in resp.headers().iter() {
        println!("{}: {}", name.to_string().green(), value.to_str().unwrap());
    }
    print!("\n");
}

/// 打印服务器返回的 HTTP body
fn print_body(m: Option<Mime>, body: &str) {
    match m {
        // 对于 "application/json" 我们 pretty print
        Some(v) if v == mime::APPLICATION_JSON => print_syntect(body, "json"),
        // 对于 "application/xml" 我们格式化输出
        Some(v) if v == mime::TEXT_HTML => print_syntect(body, "html"),
        // 其它情况，直接输出
        _ => println!("{}", body.cyan()),
    }
}

async fn print_resp(resp: Response) -> Result<()> {
    print_status(&resp);
    print_headers(&resp);
    let mime = get_content_type(&resp);
    let body = resp.text().await?;
    print_body(mime, &body);
    Ok(())
}

fn get_content_type(resp: &Response) -> Option<Mime> {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().parse().unwrap())
}

async fn get(client: Client, args: &Get) -> Result<()> {
    let resp = client.get(&args.url).send().await?;
    // 打印状态码
    // println!("{:?}", resp.status());
    // 打印返回的头部
    // println!("{:#?}", resp.headers());
    // 读取并打印返回的body
    // let body = resp.text().await?;
    // println!("{}", body);
    Ok(print_resp(resp).await?)
}

async fn post(client: Client, args: &Post) -> Result<()> {
    let mut body = HashMap::new();
    for pair in args.body.iter() {
        body.insert(&pair.k, &pair.v);
    }
    let resp = client.post(&args.url).json(&body).send().await?;
    // println!("{:?}", resp.status());
    // println!("{:#?}", resp.headers());
    // let body = resp.text().await?;
    // println!("{}", body);
    Ok(print_resp(resp).await?)
    // let mut body = serde_json::Map::new();
    // for kv in args.body.iter() {
    //     body.insert(kv.k.clone(), serde_json::json!(kv.v));
    // }
    // let resp = client
    //     .post(&args.url)
    //     .header(header::CONTENT_TYPE, "application/json")
    //     .body(serde_json::Value::Object(body).to_string())
    //     .send()
    //     .await?;
    // // 打印状态码
    // println!("{:?}", resp.status());
    // // 打印返回的头部
    // println!("{:#?}", resp.headers());
    // // 读取并打印返回的body
    // let body = resp.text().await?;
    // println!("{}", body);
    // Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let mut headers = header::HeaderMap::new();
    // 为我们的http客户端增加一些缺省的头部
    // headers.insert("X-POWERED-BY", header::HeaderValue::from_static("Rust"));
    headers.insert("X-POWERED-BY", "Rust".parse()?);
    headers.insert(header::USER_AGENT, "Rust Httpie".parse()?);
    // 生成一个 HTTP 客户端
    let client = Client::builder()
        .default_headers(headers)
        .build()?;
    let result = match opts.subcmd {
        SubCommand::Get(ref args) => get(client, args).await?,
        SubCommand::Post(ref args) => post(client, args).await?,
    };
    Ok(result)
}
