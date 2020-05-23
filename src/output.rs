use super::argparse::Printer;
use super::errors::*;
use super::subprocess::SubprocessCommand;
use super::types::Task;
use ansi_term::Colour;
use async_std::sync::Receiver;
use futures::future::try_join;
use std::{env, process};
use tokio::{
  io::{self, AsyncWriteExt, BufWriter},
  task,
};

fn stream_stdout(stream: Receiver<SadResult<String>>) -> Task {
  let mut stdout = BufWriter::new(io::stdout());
  task::spawn(async move {
    while let Some(print) = stream.recv().await {
      match print {
        Ok(val) => match stdout.write(val.as_bytes()).await {
          Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => process::exit(1),
          Err(e) => err_exit(e.into()),
          _ => {}
        },
        Err(e) => err_exit(e),
      }
    }
    stdout.shutdown().await.unwrap()
  })
}

pub fn stream_output(printer: Printer, stream: Receiver<SadResult<String>>) -> Task {
  match printer {
    Printer::Pager(cmd) => {
      let (child, rx) = cmd.stream(stream);
      let recv = stream_stdout(rx);
      task::spawn(async {
        if let Err(e) = try_join(child, recv).await {
          err_exit(e.into())
        }
      })
    }
    Printer::Fzf => {
      let args = env::args()
        .filter(|a| a != "--pick")
        .collect::<Vec<String>>()
        .join(" ");
      let cmd = SubprocessCommand {
        program: "fzf".to_string(),
        arguments: vec![
          "--read0".to_string(),
          "-m".to_string(),
          "--ansi".to_string(),
          format!("--preview={} --internal-preview={{}}", args),
          "--preview-window=70%:wrap".to_string(),
        ],
      };
      let (child, rx) = cmd.stream_connected(stream);
      let recv = stream_stdout(rx);
      task::spawn(async {
        if let Err(e) = try_join(child, recv).await {
          err_exit(e.into())
        }
      })
    }
    Printer::Stdout => stream_stdout(stream),
  }
}

pub fn err_exit(err: Failure) -> ! {
  eprintln!("{}", Colour::Red.paint(format!("{:#?}", err)));
  process::exit(1)
}