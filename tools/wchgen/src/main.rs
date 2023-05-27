use std::io::{BufRead, BufReader, Error, ErrorKind, Result};
use std::{fmt, fs, path};

const STATUSLINE: &str = "HTTP/1.1 200 OK";
const CONTENT_TYPE: &str = "Content-Type: text/html; charset=utf-8";

struct Response<'a>(&'a str);

impl<'a> fmt::Display for Response<'a> {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let len = self.0.len();
    write!(
      formatter,
      "{}\r\n{}\r\nContent-Length: {}\r\n\r\n{}",
      STATUSLINE, CONTENT_TYPE, len, self.0
    )
  }
}

fn main() -> Result<()> {
  let mut args = std::env::args().skip(1);

  let (target, html) = match args.next() {
    Some(inner) if inner.as_str() == "--html" => (args.next(), true),
    Some(other) => (Some(other), false),
    None => (None, false),
  };

  let target = target.ok_or_else(|| Error::new(ErrorKind::InvalidInput, "missing target"))?;

  let p = path::PathBuf::from(&target);

  if !p.is_file() {
    let error = format!("'{}' is not a valid file", target);
    return Err(Error::new(ErrorKind::Other, error));
  }

  let handle = fs::File::open(p)?;
  let reader = BufReader::new(handle);
  let minified = reader.lines().fold(String::with_capacity(1024), |mut out, line| match line {
    Ok(valid) => {
      out.push_str(valid.trim_start().trim_end());
      out.replace('\"', "'")
    }
    Err(_) => out,
  });

  if html {
    println!("{minified}");
    return Ok(());
  }

  let formatted = format!("{}", Response(minified.as_str()));
  println!("{}", formatted);
  eprintln!("html size: {}bytes\ntotal size: {}bytes", minified.len(), formatted.len());

  Ok(())
}
